#!/bin/bash
# Database Setup Script for Police Thief Game

echo "🚀 Police Thief Game Database Setup"
echo "=================================="

# Check if MySQL is running
if ! pgrep -x "mysqld" > /dev/null; then
    echo "❌ MySQL is not running. Please start MySQL first."
    echo "   brew services start mysql"
    exit 1
fi

echo "📊 Creating database..."
mysql -u root -p << 'EOF'
-- Create database
CREATE DATABASE IF NOT EXISTS police_thief_simple
    CHARACTER SET utf8mb4
    COLLATE utf8mb4_unicode_ci;

-- Show created database
SHOW DATABASES LIKE 'police_thief_simple';
EOF

if [ $? -eq 0 ]; then
    echo "✅ Database created successfully"
    
    echo "📋 Applying schema..."
    mysql -u root -p police_thief_simple < sql/schema_simple.sql
    
    if [ $? -eq 0 ]; then
        echo "✅ Schema applied successfully"
        
        echo "🧪 Creating test data..."
        mysql -u root -p police_thief_simple << 'EOF'
CALL sp_create_test_data();
SELECT 'Test data created successfully' as status;
EOF
        
        echo "✅ Database setup complete!"
        echo ""
        echo "📝 Connection Info:"
        echo "   Database: police_thief_simple"
        echo "   User: game_simple"
        echo "   Password: game_password_123"
        echo ""
        echo "🔗 Connection URL:"
        echo "   mysql://game_simple:game_password_123@localhost/police_thief_simple"
    else
        echo "❌ Failed to apply schema"
        exit 1
    fi
else
    echo "❌ Failed to create database"
    exit 1
fi