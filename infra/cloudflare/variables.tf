variable "cloudflare_account_id" {
  description = "Cloudflare account containing the playsrc.online zone and R2 bucket."
  type        = string
  sensitive   = true

  validation {
    condition     = can(regex("^[0-9a-f]{32}$", var.cloudflare_account_id))
    error_message = "cloudflare_account_id must be one lowercase 32-hex identifier."
  }
}

variable "cloudflare_zone_id" {
  description = "Cloudflare zone identifier for playsrc.online."
  type        = string
  sensitive   = true

  validation {
    condition     = can(regex("^[0-9a-f]{32}$", var.cloudflare_zone_id))
    error_message = "cloudflare_zone_id must be one lowercase 32-hex identifier."
  }
}
