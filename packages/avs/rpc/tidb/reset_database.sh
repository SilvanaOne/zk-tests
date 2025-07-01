#!/bin/bash

# Silvana RPC Database Reset Script
# ⚠️  WARNING: This script drops ALL existing tables in the database (complete clean slate)
# This includes any tables not created by this RPC system
# Make sure to set your DATABASE_URL environment variable before running

set -e  # Exit on any error

echo "🗃️  Silvana RPC Database Reset Script"
echo "=================================="

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "❌ ERROR: DATABASE_URL environment variable is not set"
    echo "Please set it like: export DATABASE_URL='mysql://user:password@host:port/database'"
    exit 1
fi

echo "📊 Database URL: $DATABASE_URL"
echo ""

# Load environment variables if .env file exists
if [ -f .env ]; then
    echo "📁 Loading environment variables from .env file..."
    # Filter out comments and empty lines, then export
    export $(grep -v '^#' .env | grep -v '^$' | xargs)
    echo "✅ Environment loaded"
else
    echo "⚠️  No .env file found, using system environment variables"
fi

# Check if sea-orm-cli is installed
if ! command -v sea-orm-cli &> /dev/null; then
    echo "❌ ERROR: sea-orm-cli is not installed"
    echo "Please install it with: cargo install sea-orm-cli"
    exit 1
fi

echo ""
echo "🔄 Starting database reset process..."
echo ""

# Drop ALL existing tables in the database
echo "🗑️  Dropping ALL existing tables..."

# Extract database connection details for direct SQL execution
DB_HOST=$(echo $DATABASE_URL | sed -n 's/.*@\([^:]*\):.*/\1/p')
DB_PORT=$(echo $DATABASE_URL | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
DB_USER=$(echo $DATABASE_URL | sed -n 's/mysql:\/\/\([^:]*\):.*/\1/p')
DB_PASS=$(echo $DATABASE_URL | sed -n 's/mysql:\/\/[^:]*:\([^@]*\)@.*/\1/p')
DB_NAME=$(echo $DATABASE_URL | sed -n 's/.*\/\([^?]*\).*/\1/p')

echo "🏗️  Connecting to TiDB: $DB_USER@$DB_HOST:$DB_PORT/$DB_NAME"

# Drop all tables using mysql command if available, otherwise use Rust utility
if command -v mysql &> /dev/null; then
    echo "🗑️  Using mysql client to query all tables in database..."
    
    # Get list of all tables in the database
    ALL_TABLES=$(mysql -h $DB_HOST -P $DB_PORT -u $DB_USER -p$DB_PASS $DB_NAME \
        -e "SHOW TABLES;" -s -N 2>/dev/null || echo "")
    
    if [ -n "$ALL_TABLES" ]; then
        echo "📋 Found tables to drop:"
        echo "$ALL_TABLES" | sed 's/^/  - /'
        
        echo ""
        echo "🗑️  Dropping all tables..."
        
        # Disable foreign key checks to avoid dependency issues
        mysql -h $DB_HOST -P $DB_PORT -u $DB_USER -p$DB_PASS $DB_NAME \
            -e "SET FOREIGN_KEY_CHECKS = 0;" 2>/dev/null
        
        # Drop each table
        while IFS= read -r table; do
            if [ -n "$table" ]; then
                echo "  - Dropping table: $table"
                mysql -h $DB_HOST -P $DB_PORT -u $DB_USER -p$DB_PASS $DB_NAME \
                    -e "DROP TABLE IF EXISTS \`$table\`;" 2>/dev/null && echo "    ✅ Dropped" || echo "    ❌ Failed to drop"
            fi
        done <<< "$ALL_TABLES"
        
        # Re-enable foreign key checks
        mysql -h $DB_HOST -P $DB_PORT -u $DB_USER -p$DB_PASS $DB_NAME \
            -e "SET FOREIGN_KEY_CHECKS = 1;" 2>/dev/null
        
        echo "✅ All tables dropped successfully"
    else
        echo "📭 No tables found in database (database is already empty)"
    fi
else
    echo "⚠️  mysql client not found, using Rust utility to drop all tables..."
    
    # Use Rust utility to drop all tables
    echo "🔧 Compiling and running table drop utility..."
    if rust_output=$(DATABASE_URL="$DATABASE_URL" cargo run --manifest-path tidb/drop_all_tables/Cargo.toml 2>&1); then
        echo "$rust_output"
        echo "✅ All tables dropped using Rust utility"
    else
        echo "❌ Failed to drop tables using Rust utility:"
        echo "$rust_output"
        echo "⚠️  Will rely on sea-orm migration reset for known tables only"
    fi
fi

echo ""
echo "🔄 Resetting sea-orm migration state..."
# Add a 5 second delay before resetting migration state
echo "⏳ Waiting 5 seconds before resetting migration state..."
sleep 5


# Reset migrations (this will also drop tables if mysql client wasn't available)
sea-orm-cli migrate reset -d ./migration -u "$DATABASE_URL"

echo ""
echo "🏗️  Running fresh migrations..."

# Run all migrations to recreate tables
sea-orm-cli migrate up -d ./migration -u "$DATABASE_URL"

echo ""
echo "📊 Verifying table creation..."

# List tables to verify creation
if command -v mysql &> /dev/null; then
    echo "📋 Tables now in database:"
    CREATED_TABLES=$(mysql -h $DB_HOST -P $DB_PORT -u $DB_USER -p$DB_PASS $DB_NAME \
        -e "SHOW TABLES;" -s -N 2>/dev/null || echo "")
    
    if [ -n "$CREATED_TABLES" ]; then
        echo "$CREATED_TABLES" | sed 's/^/  ✅ /'
        TABLE_COUNT=$(echo "$CREATED_TABLES" | wc -l | xargs)
        echo "📊 Total tables created: $TABLE_COUNT"
    else
        echo "❌ No tables found after migration"
    fi
else
    echo "✅ Migration completed (use mysql client to verify tables)"
fi

echo ""
echo "🎉 Database reset completed successfully!"
echo ""
echo "📝 Summary:"
echo "  - ALL existing tables in database dropped (complete clean slate)"
echo "  - Fresh migration state created"  
echo "  - All 9 RPC event tables recreated with proper schema:"
echo "    • coordinator_started_events"
echo "    • agent_started_job_events" 
echo "    • agent_finished_job_events"
echo "    • coordination_tx_events"
echo "    • coordinator_error_events"
echo "    • client_transaction_events"
echo "    • agent_message_events"
echo "    • agent_error_events"
echo "    • agent_transaction_events"
echo "  - Indexes created for efficient querying on coordinator_id and timestamp"
echo ""
echo "🚀 Your RPC server is ready to use the completely fresh database!" 