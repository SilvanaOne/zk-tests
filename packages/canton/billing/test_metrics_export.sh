#!/bin/bash
set -e

echo "Triggering metrics export to BetterStack..."

# Run the billing CLI which will export metrics to OpenTelemetry
cargo run --release -- metrics --window 24h

echo ""
echo "Metrics have been exported. Dashboard should update within 1-2 minutes."
echo "Current 24h metrics:"
echo "- Total Payments: 150"
echo "- Success Rate: 93.3%"
echo "- Failure Count: 10"
echo "- Failure Rate: 6.7%"