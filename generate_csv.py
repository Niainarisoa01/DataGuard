import csv
import random

def generate_csv(filename, num_rows):
    print(f"Generating {num_rows} rows into {filename}...")
    with open(filename, 'w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)
        writer.writerow(['username', 'age'])
        
        for i in range(num_rows):
            # 10% chance to generate an invalid row (age < 18 or empty username)
            if random.random() < 0.1:
                writer.writerow(['invalid_user', random.randint(5, 17)])
            else:
                writer.writerow([f'user_{i}', random.randint(18, 99)])
                
    print(f"Generated {filename}")

if __name__ == '__main__':
    generate_csv('/tmp/massive_test.csv', 200_000)
