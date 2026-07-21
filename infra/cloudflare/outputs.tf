output "asset_bucket" {
  description = "R2 bucket receiving immutable playsrc objects."
  value       = cloudflare_r2_bucket.assets.name
}

output "asset_origin" {
  description = "Public immutable-object origin."
  value       = "https://${cloudflare_r2_custom_domain.assets.domain}"
}
