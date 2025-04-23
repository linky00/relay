CREATE TABLE "messages" (
    "id" INTEGER NOT NULL UNIQUE,
    "from_key" TEXT NOT NULL,
    "signature" TEXT NOT NULL UNIQUE,
    "uuid" TEXT NOT NULL UNIQUE,
    "author" TEXT NOT NULL,
    "line" TEXT NOT NULL,
    "received_at" INTEGER NOT NULL,
    PRIMARY KEY("id" AUTOINCREMENT)
);
CREATE TABLE "envelopes" (
    "id" INTEGER NOT NULL UNIQUE,
    "from_key" TEXT NOT NULL,
    "ttl" INTEGER NOT NULL,
    "received_at" INTEGER NOT NULL,
    "message_id" INTEGER NOT NULL,
    PRIMARY KEY("id" AUTOINCREMENT),
    FOREIGN KEY("message_id") REFERENCES "messages"("id")
);
CREATE TABLE "forwards" (
    "from_key" TEXT NOT NULL,
    "envelope_id" INTEGER NOT NULL,
    PRIMARY KEY("from_key", "envelope_id"),
    FOREIGN KEY("envelope_id") REFERENCES "envelopes"("id")
);