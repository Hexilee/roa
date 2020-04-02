```bash
RUST_LOG=info cargo run --bin api
```

- `curl 127.0.0.1:8000/post/1`
    query post where id=0 and published
- `curl -H "Content-type: application/json" -d '{"title":"Hello", "body": "Hello, world", "published": false}' -X POST 127.0.0.1:8000/post`
    create a new post
- `curl -H "Content-type: application/json" -d '{"title":"Hello", "body": "Hello, world", "published": true}' -X PUT 127.0.0.1:8000/post/1`
    update post where id=0, return the old data
- `curl 127.0.0.1:8000/post/1 -X DELETE`
    delete post where id=0
