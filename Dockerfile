FROM rust

WORKDIR /app
COPY . .

ENV SQLX_OFFLINE=true
RUN cargo install --path ./relay_textfiles

WORKDIR /

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 7070
VOLUME /config /store

ENTRYPOINT ["/entrypoint.sh"]