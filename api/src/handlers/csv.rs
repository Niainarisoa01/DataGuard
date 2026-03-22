use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, Response, StatusCode},
};
use bytes::Bytes;
use csv_async::AsyncReaderBuilder;
use jsonschema::JSONSchema;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use tokio_util::compat::TokioAsyncReadCompatExt;
use uuid::Uuid;

use crate::AppState;

pub async fn validate_csv(
    State(state): State<AppState>,
    Path(schema_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Response<Body>, StatusCode> {
    // Retrieve schema from cache or DB BEFORE starting the stream
    let schema_json = if let Some(cached) = state.schema_cache.get(&schema_id).await {
        cached
    } else {
        let record = sqlx::query!(
            "SELECT json_schema FROM schema_versions WHERE schema_id = $1 ORDER BY version DESC LIMIT 1",
            schema_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match record {
            Some(r) => {
                let arc_schema = Arc::new(r.json_schema);
                state.schema_cache.insert(schema_id, arc_schema.clone()).await;
                arc_schema
            }
            None => return Err(StatusCode::NOT_FOUND),
        }
    };

    let compiled_schema =
        JSONSchema::compile(&schema_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Channel for streaming out the CSV
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::convert::Infallible>>(1000);

    let db = state.db.clone();
    let schema_id_uuid = schema_id;
    let start_time = std::time::Instant::now();

    tokio::spawn(async move {
        // Send header
        let _ = tx.send(Ok(Bytes::from("row_number,errors,raw_data\n"))).await;

        while let Ok(Some(mut field)) = multipart.next_field().await {
            if field.name() == Some("file") {
                let stream = async_stream::stream! {
                    while let Some(chunk_res) = field.chunk().await.transpose() {
                        match chunk_res {
                            Ok(chunk) => yield std::io::Result::Ok(chunk),
                            Err(e) => {
                                let err_msg = format!("Extrait Multipart coupé ou rejeté: {}", e);
                                yield std::io::Result::Err(std::io::Error::new(std::io::ErrorKind::Other, err_msg));
                            }
                        }
                    }
                };
                tokio::pin!(stream);
                let reader = tokio_util::io::StreamReader::new(stream).compat();

                let mut csv_reader = AsyncReaderBuilder::new().create_deserializer(reader);
                let mut iter =
                    csv_reader.deserialize::<std::collections::HashMap<String, String>>();

                let mut row_num = 1;
                while let Some(result) = tokio_stream::StreamExt::next(&mut iter).await {
                    match result {
                        Ok(hashmap) => {
                            let mut json_map = serde_json::Map::new();
                            for (k, v) in hashmap {
                                let json_val = if let Ok(n) = v.parse::<i64>() {
                                    serde_json::Value::Number(n.into())
                                } else if let Ok(f) = v.parse::<f64>() {
                                    serde_json::Value::Number(
                                        serde_json::Number::from_f64(f).unwrap(),
                                    )
                                } else if let Ok(b) = v.parse::<bool>() {
                                    serde_json::Value::Bool(b)
                                } else {
                                    serde_json::Value::String(v)
                                };
                                json_map.insert(k, json_val);
                            }
                            let row = serde_json::Value::Object(json_map);

                            if !compiled_schema.is_valid(&row) {
                                if let Err(errs) = compiled_schema.validate(&row) {
                                    let errors_str: Vec<String> =
                                        errs.map(|e| e.to_string()).collect();
                                    let errors_joined = errors_str.join("; ");

                                    let raw_json = serde_json::to_string(&row)
                                        .unwrap_or_default()
                                        .replace("\"", "\"\"");
                                    let err_msg = errors_joined.replace("\"", "\"\"");

                                    let line =
                                        format!("{},\"{}\",\"{}\"\n", row_num, err_msg, raw_json);
                                    let _ = tx.send(Ok(Bytes::from(line))).await;
                                }
                            }
                        }
                        Err(e) => {
                            let parse_err: csv_async::Error = e;
                            let line = format!(
                                "{},\"Parse error: {}\",\"\"\n",
                                row_num,
                                parse_err.to_string().replace("\"", "\"\"")
                            );
                            let _ = tx.send(Ok(Bytes::from(line))).await;
                        }
                    }
                    row_num += 1;
                }

                if row_num > 1 {
                    let total_records = (row_num - 1) as i32;
                    let duration = start_time.elapsed().as_millis() as i32;
                    let _ = sqlx::query!(
                        "INSERT INTO usage_logs (account_id, schema_id, endpoint, status_code, is_valid, duration_ms, records_processed) 
                         VALUES ((SELECT account_id FROM schemas WHERE id = $1), $1, $2, $3, $4, $5, $6)",
                         schema_id_uuid,
                         "/v1/validate/csv",
                         200,    // status_code HTTP OK
                         true,   // API call itself is valid
                         duration,
                         total_records
                    )
                    .execute(&db)
                    .await;
                }

                return; // Done processing
            }
        }

        let _ = tx.send(Ok(Bytes::from("0,\"Error: No file field found\",\"\"\n"))).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"errors.csv\"")
        .body(body)
        .unwrap();

    Ok(response)
}
