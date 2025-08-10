# Police Thief Game Server Docker Management Makefile

.PHONY: help build up down restart logs clean test unified micro

# Default target
help: ## Show this help message
	@echo "Police Thief Game Server Docker Commands"
	@echo "========================================"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# Environment setup
setup: ## Setup environment files
	@echo "Setting up environment..."
	@cp .env.example .env
	@echo "✅ Environment file created. Please edit .env with your configuration."

# Build commands
build: ## Build all Docker images
	@echo "Building all Docker images..."
	@docker-compose -f docker-compose.unified.yml build
	@docker-compose -f docker-compose.microservices.yml build
	@echo "✅ All images built successfully"

build-unified: ## Build unified server image only
	@echo "Building unified server image..."
	@docker build -f unified/Dockerfile -t police-thief-unified ../../../
	@echo "✅ Unified server image built"

build-micro: ## Build all microservice images
	@echo "Building microservice images..."
	@docker build -f grpc/Dockerfile -t police-thief-grpc ../../../
	@docker build -f tcp/Dockerfile -t police-thief-tcp ../../../
	@docker build -f rudp/Dockerfile -t police-thief-rudp ../../../
	@echo "✅ All microservice images built"

# Deployment commands
unified: ## Start unified server (recommended)
	@echo "Starting unified Police Thief server..."
	@docker-compose -f docker-compose.unified.yml up -d
	@echo "✅ Unified server started on ports: gRPC(50051), TCP(4000), RUDP(5000)"
	@$(MAKE) status-unified

micro: ## Start microservices
	@echo "Starting Police Thief microservices..."
	@docker-compose -f docker-compose.microservices.yml up -d
	@echo "✅ Microservices started"
	@$(MAKE) status-micro

# Management commands
up-unified: ## Start unified server in foreground
	@docker-compose -f docker-compose.unified.yml up

up-micro: ## Start microservices in foreground
	@docker-compose -f docker-compose.microservices.yml up

down: ## Stop all services
	@echo "Stopping all Police Thief services..."
	@docker-compose -f docker-compose.unified.yml down 2>/dev/null || true
	@docker-compose -f docker-compose.microservices.yml down 2>/dev/null || true
	@echo "✅ All services stopped"

restart: ## Restart current services
	@echo "Restarting services..."
	@$(MAKE) down
	@sleep 2
	@$(MAKE) unified

restart-micro: ## Restart microservices
	@echo "Restarting microservices..."
	@docker-compose -f docker-compose.microservices.yml restart
	@echo "✅ Microservices restarted"

# Monitoring commands
logs: ## Show logs from all services
	@docker-compose -f docker-compose.unified.yml logs -f 2>/dev/null || \
	 docker-compose -f docker-compose.microservices.yml logs -f

logs-unified: ## Show unified server logs
	@docker-compose -f docker-compose.unified.yml logs -f

logs-micro: ## Show microservices logs
	@docker-compose -f docker-compose.microservices.yml logs -f

logs-grpc: ## Show gRPC service logs
	@docker logs police-grpc -f

logs-tcp: ## Show TCP service logs
	@docker logs police-tcp -f

logs-rudp: ## Show RUDP service logs  
	@docker logs police-rudp -f

logs-redis: ## Show Redis logs
	@docker logs police-redis -f

# Status commands
status: ## Show status of all services
	@$(MAKE) status-unified 2>/dev/null || $(MAKE) status-micro

status-unified: ## Show unified server status
	@echo "Unified Server Status:"
	@echo "====================="
	@docker-compose -f docker-compose.unified.yml ps
	@echo ""
	@echo "Health Status:"
	@docker exec police-gamecenter ./health-check.sh 2>/dev/null && echo "✅ Unified server healthy" || echo "❌ Unified server unhealthy"

status-micro: ## Show microservices status
	@echo "Microservices Status:"
	@echo "===================="
	@docker-compose -f docker-compose.microservices.yml ps

health: ## Check health of all services
	@echo "Checking service health..."
	@echo "Redis: $$(docker exec police-redis redis-cli ping 2>/dev/null || echo 'DOWN')"
	@echo "gRPC: $$(curl -s -f http://localhost:50051/health >/dev/null && echo 'UP' || echo 'DOWN')"
	@echo "TCP: $$(echo 'PING' | nc -w 1 localhost 4000 >/dev/null 2>&1 && echo 'UP' || echo 'DOWN')"
	@echo "RUDP: $$(ss -uln | grep :5000 >/dev/null && echo 'UP' || echo 'DOWN')"

# Scaling commands
scale-grpc: ## Scale gRPC service (usage: make scale-grpc REPLICAS=3)
	@docker-compose -f docker-compose.microservices.yml up -d --scale grpc-service=${REPLICAS:-2}
	@echo "✅ gRPC service scaled to ${REPLICAS:-2} instances"

scale-tcp: ## Scale TCP service (usage: make scale-tcp REPLICAS=3)
	@docker-compose -f docker-compose.microservices.yml up -d --scale tcp-service=${REPLICAS:-2}
	@echo "✅ TCP service scaled to ${REPLICAS:-2} instances"

# Development commands
dev: ## Start services in development mode
	@echo "Starting in development mode..."
	@docker-compose -f docker-compose.unified.yml -f docker-compose.dev.yml up -d
	@echo "✅ Development environment started"

shell: ## Access unified server container shell
	@docker exec -it police-gamecenter bash

shell-redis: ## Access Redis container shell
	@docker exec -it police-redis redis-cli

# Testing commands
test: ## Run health checks and basic tests
	@echo "Running basic health checks..."
	@$(MAKE) health
	@echo ""
	@echo "Testing connectivity..."
	@curl -s -f http://localhost:50051/health >/dev/null && echo "✅ gRPC endpoint reachable" || echo "❌ gRPC endpoint failed"
	@echo "PING" | nc -w 1 localhost 4000 >/dev/null 2>&1 && echo "✅ TCP endpoint reachable" || echo "❌ TCP endpoint failed"

test-load: ## Run basic load test (requires test tools)
	@echo "Running basic load test..."
	@echo "This would run load tests - implement with your testing tools"

# Cleanup commands
clean: ## Remove all containers and volumes
	@echo "Cleaning up all Police Thief containers and volumes..."
	@docker-compose -f docker-compose.unified.yml down -v --remove-orphans 2>/dev/null || true
	@docker-compose -f docker-compose.microservices.yml down -v --remove-orphans 2>/dev/null || true
	@docker system prune -f
	@echo "✅ Cleanup completed"

clean-images: ## Remove all built images
	@echo "Removing all Police Thief Docker images..."
	@docker rmi police-thief-unified police-thief-grpc police-thief-tcp police-thief-rudp 2>/dev/null || true
	@echo "✅ Images removed"

# Backup commands
backup-redis: ## Backup Redis data
	@echo "Creating Redis backup..."
	@docker exec police-redis redis-cli BGSAVE
	@docker cp police-redis:/data/dump.rdb ./backup-$(shell date +%Y%m%d_%H%M%S).rdb
	@echo "✅ Redis backup created"

# Monitoring commands
monitor: ## Open monitoring dashboard
	@echo "Opening monitoring dashboard..."
	@open http://localhost:9090 || xdg-open http://localhost:9090 || echo "Please open http://localhost:9090 manually"

# Quick commands for common operations
start: unified ## Alias for unified (quick start)
stop: down ## Alias for down (quick stop)
restart-quick: restart ## Alias for restart