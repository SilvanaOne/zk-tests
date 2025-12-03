#!/bin/bash
set -e

echo "=== Cloud Signer Deployment ==="

# Enable required APIs
echo "Enabling APIs..."
gcloud services enable secretmanager.googleapis.com --quiet
gcloud services enable run.googleapis.com --quiet
gcloud services enable containerregistry.googleapis.com --quiet

# Create/update secrets in Secret Manager
echo "Creating secrets in Secret Manager..."
echo -n "$SIGNER_NAME" | gcloud secrets create SIGNER_NAME --data-file=- --replication-policy=automatic 2>/dev/null || \
echo -n "$SIGNER_NAME" | gcloud secrets versions add SIGNER_NAME --data-file=-

echo -n "$SIGNER_PRIVATE_KEY" | gcloud secrets create SIGNER_PRIVATE_KEY --data-file=- --replication-policy=automatic 2>/dev/null || \
echo -n "$SIGNER_PRIVATE_KEY" | gcloud secrets versions add SIGNER_PRIVATE_KEY --data-file=-

# Grant Cloud Run service account access to secrets
echo "Granting secret access to Cloud Run service account..."
PROJECT_NUMBER=$(gcloud projects describe "$GOOGLE_CLOUD_PROJECT" --format='value(projectNumber)')
SERVICE_ACCOUNT="${PROJECT_NUMBER}-compute@developer.gserviceaccount.com"

gcloud secrets add-iam-policy-binding SIGNER_NAME \
  --member="serviceAccount:${SERVICE_ACCOUNT}" \
  --role="roles/secretmanager.secretAccessor" --quiet

gcloud secrets add-iam-policy-binding SIGNER_PRIVATE_KEY \
  --member="serviceAccount:${SERVICE_ACCOUNT}" \
  --role="roles/secretmanager.secretAccessor" --quiet

# Build and push image
echo "Building container image..."
IMAGE="gcr.io/$GOOGLE_CLOUD_PROJECT/$K_SERVICE"
docker build -t "$IMAGE" .
docker push "$IMAGE"

# Deploy to Cloud Run - the app reads secrets from Secret Manager at runtime
echo "Deploying to Cloud Run..."
gcloud run deploy "$K_SERVICE" \
  --image="$IMAGE" \
  --region="$GOOGLE_CLOUD_REGION" \
  --platform=managed \
  --allow-unauthenticated \
  --set-env-vars="GCP_PROJECT_ID=$GOOGLE_CLOUD_PROJECT"

echo "=== Deployment complete ==="
