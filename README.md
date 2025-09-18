wip. not ready to be published on cargo, and perhaps never will be. <3

## sqlx

to prepare sqlx:

1. set `DATABASE_URL` in `.env` to an **absolute** sqlite path, such as `sqlite:///tmp/test.db`. don't set `SQLX_OFFLINE` yet.

2. run:

```
cargo sqlx db setup --source relay_daemon/migrations/
cargo sqlx prepare --workspace
```

3. now you can set `SQLX_OFFLINE=true` in `.env`.

