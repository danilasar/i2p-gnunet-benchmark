import json

def load_jsonl(path: str) -> list[dict]:
    """Загружает JSONL-лог из path, возвращает список записей."""
    data = []
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            if line.strip():
                data.append(json.loads(line))
    return data

if __name__ == "__main__":
    print("Analyzer stub")
