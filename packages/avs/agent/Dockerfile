# syntax=docker/dockerfile:1

###############################################################################
# Stage 0 – build & bundle
###############################################################################
FROM node:22-alpine 
# install dependencies and build TS → JS
WORKDIR /app
COPY package.json tsconfig.json ./
COPY src ./src/
RUN npm install
RUN npm run build        

###############################################################################
# Stage 1 – flattened runtime (ONE layer)
###############################################################################
# FROM astefanutti/scratch-node
# COPY --from=builder /app /app
# WORKDIR /app
CMD ["npm", "run", "start"]