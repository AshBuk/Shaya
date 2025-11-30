#!/bin/bash
# Copyright 2025 Asher Buk
#
# SPDX-License-Identifier: Apache-2.0
#
# https://github.com/AshBuk/ShyQuiet

# Helper script for Docker build workflow

set -e

cd "$(dirname "$0")"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== ShyQuiet Docker Build ===${NC}\n"

print_step() { echo -e "${YELLOW}>>> $1${NC}"; }
print_success() { echo -e "${GREEN}✓ $1${NC}"; }
print_error() { echo -e "${RED}✗ $1${NC}"; }

# Check docker
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed."
    exit 1
fi

# Docker compose command
if docker compose version &> /dev/null; then
    DC="docker compose"
elif command -v docker-compose &> /dev/null; then
    DC="docker-compose"
else
    print_error "Docker Compose not available."
    exit 1
fi

echo "Select an option:"
echo "1) Build Docker image"
echo "2) Start container (shell)"
echo "3) Build ShyQuiet (cargo build --release)"
echo "4) Check compilation (cargo check)"
echo "5) Stop container"
echo "6) Clean all"
echo ""
read -p "Enter choice [1-6]: " choice

case $choice in
    1)
        print_step "Building Docker image..."
        $DC build --no-cache
        print_success "Docker image built!"
        ;;
    2)
        print_step "Starting container..."
        $DC up -d
        print_success "Container started. Entering shell..."
        $DC exec shyquiet /bin/bash
        ;;
    3)
        print_step "Building ShyQuiet (GTK4 + libadwaita)..."
        $DC up -d
        $DC exec shyquiet bash -c "cargo build --release"
        print_success "Build completed! Check target/release/shyquiet"
        ;;
    4)
        print_step "Checking compilation..."
        $DC up -d
        $DC exec shyquiet bash -c "cargo check"
        print_success "Compilation check passed!"
        ;;
    5)
        print_step "Stopping container..."
        $DC down
        print_success "Container stopped!"
        ;;
    6)
        print_step "Cleaning all..."
        read -p "Remove all volumes and images? (y/N): " confirm
        if [[ $confirm == [yY] ]]; then
            $DC down -v
            docker rmi shyquiet-builder 2>/dev/null || true
            print_success "Cleaned!"
        fi
        ;;
    *)
        print_error "Invalid choice"
        exit 1
        ;;
esac

echo ""
print_success "Done!"
