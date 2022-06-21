terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 4.0"
    }
  }

  required_version = ">= 0.14.9"
}

provider "aws" {
  region  = "eu-west-2"
  profile = "shuttle-dev"
}

module "shuttle" {
  source = "../terraform/modules/shuttle"

  api_fqdn             = "api.test.shuttle.rs"
  pg_fqdn              = "pg.test.shuttle.rs"
  postgres_password    = "password"
  proxy_fqdn           = "test.shuttleapp.rs"
  shuttle_admin_secret = "12345"
}

output "api_url" {
  value       = module.shuttle.api_url
  description = "URL to connect to the api"
}

output "api_name_servers" {
  value = module.shuttle.api_name_servers
}

output "user_name_servers" {
  value = module.shuttle.user_name_servers
}

output "api_content_host" {
  value = module.shuttle.api_content_host
}

output "user_content_host" {
  value = module.shuttle.user_content_host
}

output "initial_user_key" {
  value       = module.shuttle.initial_user_key
  description = "Key given to the initial shuttle user"
}
