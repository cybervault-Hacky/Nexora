# Build the existing isolated website workspace. The backend itself has no
# third-party runtime dependencies; only the built site and server modules are
# needed in the final image.
FROM node:22-bookworm-slim AS build
WORKDIR /app
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
RUN corepack enable
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm build

FROM node:22-bookworm-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production \
    NEXORA_PORTAL_ENV=production \
    NEXORA_PORTAL_SECURE_COOKIES=true \
    NEXORA_PORTAL_TRUST_PROXY=false
COPY --from=build /app/dist ./dist
COPY --from=build /app/server ./server
COPY --from=build /app/package.json ./package.json
RUN mkdir -p /data && chown -R node:node /app /data
USER node
EXPOSE 4174
CMD ["node", "server/index.mjs"]
