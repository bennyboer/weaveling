#!/bin/sh
set -e

for owned in /repo/features/*/; do
    named=$(basename "$owned")

    [ -d "$owned/core" ] || continue

    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
        -c "CREATE DATABASE weaveling_$named"
done
