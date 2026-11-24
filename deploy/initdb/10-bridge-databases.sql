-- One database per bridge, named after its compose service. Postgres runs this only when it creates a new data
-- volume; POSTGRES_DB already created "telegram". A bridge added later to an existing stack needs its database
-- created by hand (docs/deploy.md).
CREATE DATABASE signal;
