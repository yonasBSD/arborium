#!/bin/bash
set -euo pipefail

# Deploy script
echo "Starting deployment..."

for service in api web worker; do
    echo "Deploying $service..."
    docker-compose up -d "$service"
done

echo "Deployment complete!"
