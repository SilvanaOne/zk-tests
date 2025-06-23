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
```
