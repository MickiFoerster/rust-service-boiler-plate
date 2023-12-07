set -e;
export POSTGRES_USER="postgres";
export POSTGRES_PASSWORD="password";
export POSTGRES_HOST=localhost;
export POSTGRES_PORT=5432;
export PGUSER=${POSTGRES_USER};
export PGPASSWORD=${POSTGRES_PASSWORD};
export PGHOST=${POSTGRES_HOST};
export DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@${POSTGRES_HOST}:${POSTGRES_PORT}/postgres;
if docker ps | grep --color=auto -q trash_db; then
    docker kill trash_db;
fi;
docker run -d -e POSTGRES_USER=${POSTGRES_USER} -e POSTGRES_PASSWORD=${POSTGRES_PASSWORD} -e POSTGRES_HOST=${POSTGRES_HOST} -e POSTGRES_PORT=${POSTGRES_PORT} -p ${POSTGRES_PORT}:${POSTGRES_PORT} --rm --name trash_db postgres -N 1024 -B 4096;
while ! pg_isready; do
    echo "database is not ready yet. Take a nap ...";
    sleep 1;
done;
if [[ -d migrations ]]; then
    echo "cargo sqlx database setup" | bash;
fi
