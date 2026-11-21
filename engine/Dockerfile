# D.A.D engine: a stateless IMathAS that renders, scores and self-tests one question at a time.
# No database, no login. Question code is untrusted: run with --network none, --read-only,
# a tmpfs /tmp and CPU/memory caps (see engine/run.sh).
FROM php:8.2-cli

ARG IMATHAS_REPO=https://github.com/drlippman/IMathAS.git
# Pinned upstream commit (2026-11-19). Bump deliberately and re-run the regression.
ARG IMATHAS_COMMIT=e679aafc20714f9af61a12c86b5ad2320d962793

RUN apt-get update \
 && apt-get install -y --no-install-recommends git ca-certificates \
 && git init /imathas && cd /imathas \
 && git remote add origin "$IMATHAS_REPO" \
 && git fetch --depth 1 origin "$IMATHAS_COMMIT" \
 && git checkout --quiet FETCH_HEAD \
 && rm -rf .git \
 && apt-get purge -y git && apt-get autoremove -y && rm -rf /var/lib/apt/lists/*

COPY php.ini /usr/local/etc/php/conf.d/dad-engine.ini
COPY serve.php momformat.php /imathas/

WORKDIR /imathas
USER www-data
EXPOSE 8080
CMD ["php", "-S", "0.0.0.0:8080", "serve.php"]
