# Image de pswd-server (docs/SERVEUR.md, § 7). Depuis la racine du dépôt :
#   docker build -f crates/pswd-server/docker/Dockerfile -t pswd-server .

FROM rust:1-slim-trixie AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p pswd-server

FROM debian:trixie-slim
# Utilisateur fixe (10001) : le dossier des données, sur la machine, doit lui appartenir.
RUN groupadd --system --gid 10001 pswd-server \
    && useradd --system --uid 10001 --gid 10001 --no-create-home \
        --home-dir /var/lib/pswd-server --shell /usr/sbin/nologin pswd-server \
    && install -d -o pswd-server -g pswd-server -m 0700 /var/lib/pswd-server /sauvegardes
COPY --from=build /src/target/release/pswd-server /usr/bin/pswd-server
USER pswd-server
EXPOSE 8443
ENTRYPOINT ["pswd-server"]
CMD ["serve"]
