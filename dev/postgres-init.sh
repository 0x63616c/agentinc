#!/bin/sh
set -eu

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<SQL
CREATE ROLE agentinc LOGIN PASSWORD 'agentinc';
CREATE DATABASE agentinc_${AINC_DB_SUFFIX} OWNER agentinc;
REVOKE CONNECT ON DATABASE agentinc_${AINC_DB_SUFFIX} FROM PUBLIC;
SQL
