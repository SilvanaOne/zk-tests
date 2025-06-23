# Config

nginx

```
server {
    listen       80;
    server_name  example.com www.example.com;

    # 1) Serve ACME HTTP-01 challenge files
    location ^~ /.well-known/acme-challenge/ {
        alias /var/www/letsencrypt/.well-known/acme-challenge/;
        # Make sure permissions allow nginx to read those files:
        #   mkdir -p /var/www/letsencrypt/.well-known/acme-challenge
        #   chown -R www-data:www-data /var/www/letsencrypt
    }

    # 2) Everything else → either block or redirect to HTTPS
    location / {
        return 301 https://$host$request_uri;
        # Or, to outright forbid:
        # return 404;
    }
}

# HTTPS server block forwarding traffic to the internal application on port 3000

server {
    listen 443 ssl;
    server_name  example.com www.example.com;

    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```
