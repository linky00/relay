FROM rust

WORKDIR /app
COPY . .

ENV SQLX_OFFLINE=true
RUN cargo install --path ./relay_textfiles

WORKDIR /

RUN relayt init-store store

EXPOSE 7070

CMD ["relayt", "start", "config", "store"]