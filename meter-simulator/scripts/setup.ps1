# Meter Simulator Setup Script (PowerShell)
# This script sets up the meter simulator for development

Write-Host "🚀 Setting up Meter Simulator..." -ForegroundColor Green

# Check if Node.js is installed
try {
    $nodeVersion = node -v
    Write-Host "✅ Node.js version $nodeVersion detected" -ForegroundColor Green
} catch {
    Write-Host "❌ Node.js is not installed. Please install Node.js 16+ first." -ForegroundColor Red
    exit 1
}

# Check Node.js version
$versionNumber = $nodeVersion -replace 'v', ''
$requiredVersion = "16.0.0"

if ([version]$versionNumber -lt [version]$requiredVersion) {
    Write-Host "❌ Node.js version $versionNumber is too old. Please upgrade to 16+." -ForegroundColor Red
    exit 1
}

# Install dependencies
Write-Host "📦 Installing dependencies..." -ForegroundColor Blue
npm install

# Create .env file if it doesn't exist
if (-not (Test-Path ".env")) {
    Write-Host "📝 Creating .env file from template..." -ForegroundColor Blue
    Copy-Item ".env.example" ".env"
    Write-Host "⚠️  Please edit .env file with your configuration" -ForegroundColor Yellow
}

# Create logs directory
if (-not (Test-Path "logs")) {
    New-Item -ItemType Directory -Path "logs" | Out-Null
}

# Generate test keys
Write-Host "🔑 Generating test device keys..." -ForegroundColor Blue
node src/index.js generate-keys --output test-device-keys.json

Write-Host "✅ Setup complete!" -ForegroundColor Green
Write-Host ""
Write-Host "📋 Next steps:" -ForegroundColor Cyan
Write-Host "1. Edit .env file with your configuration" -ForegroundColor White
Write-Host "2. Register a meter: node src/index.js register --keys test-device-keys.json" -ForegroundColor White
Write-Host "3. Start simulation: node src/index.js simulate --config meter-config.json" -ForegroundColor White
Write-Host ""
Write-Host "📖 For more information, see README.md" -ForegroundColor Cyan
