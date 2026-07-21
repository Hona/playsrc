locals {
  asset_bucket       = "playsrc-production-assets"
  asset_domain       = "assets.playsrc.online"
  application_origin = "https://playsrc.online"
  immutable_seconds  = 31536000
}

resource "cloudflare_r2_bucket" "assets" {
  account_id    = var.cloudflare_account_id
  name          = local.asset_bucket
  storage_class = "Standard"
}

resource "cloudflare_r2_custom_domain" "assets" {
  account_id  = var.cloudflare_account_id
  bucket_name = cloudflare_r2_bucket.assets.name
  domain      = local.asset_domain
  enabled     = true
  zone_id     = var.cloudflare_zone_id
  min_tls     = "1.2"
}

resource "cloudflare_r2_bucket_cors" "assets" {
  account_id  = var.cloudflare_account_id
  bucket_name = cloudflare_r2_bucket.assets.name

  rules = [{
    id = "playsrc-web-read"
    allowed = {
      methods = ["GET", "HEAD"]
      origins = [local.application_origin]
      headers = ["Range"]
    }
    expose_headers = [
      "Accept-Ranges",
      "CF-Cache-Status",
      "Content-Length",
      "Content-Range",
      "ETag",
    ]
    max_age_seconds = 7200
  }]
}

resource "cloudflare_ruleset" "asset_cache" {
  zone_id     = var.cloudflare_zone_id
  name        = "playsrc immutable asset cache"
  description = "Cache only canonical immutable playsrc CAS objects."
  kind        = "zone"
  phase       = "http_request_cache_settings"

  rules = [{
    ref         = "playsrc_immutable_assets"
    description = "Cache successful immutable object responses for one year."
    expression  = "(http.host eq \"${local.asset_domain}\" and starts_with(http.request.uri.path, \"/objects/sha256/\"))"
    action      = "set_cache_settings"
    action_parameters = {
      cache = true
      edge_ttl = {
        mode    = "override_origin"
        default = 0
        status_code_ttl = [
          {
            status_code_range = {
              from = 200
              to   = 299
            }
            value = local.immutable_seconds
          },
          {
            status_code_range = {
              from = 300
              to   = 599
            }
            value = -1
          },
        ]
      }
      browser_ttl = {
        mode = "respect_origin"
      }
      respect_strong_etags = true
    }
  }]
}

resource "cloudflare_tiered_cache" "assets" {
  zone_id = var.cloudflare_zone_id
  value   = "on"
}
