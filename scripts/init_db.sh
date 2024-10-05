export DATABASE_URL="sqlite:zero2prod.db"

sqlx db create -D 'sqlite:zero2prod.db'
sqlx migrate run -D 'sqlite:zero2prod.db'