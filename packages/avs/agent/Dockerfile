# syntax=docker/dockerfile:1

FROM node:22-alpine 
# install dependencies and build TS → JS
WORKDIR /app
COPY package.json tsconfig.json ./
COPY src ./src/
COPY cache ./cache
RUN npm install
RUN npm run build        

CMD ["npm", "run", "start"]