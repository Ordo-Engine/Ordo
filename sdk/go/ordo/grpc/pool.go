package grpc

import (
	"google.golang.org/grpc"
	"google.golang.org/grpc/keepalive"
	"time"
)

// PoolConfig defines gRPC connection pool configuration
type PoolConfig struct {
	// Connection pool settings
	MaxConnections int

	// Keep-alive settings
	KeepAliveTime    time.Duration
	KeepAliveTimeout time.Duration

	// Connection timeout
	ConnectTimeout time.Duration
}

// DefaultPoolConfig returns default pool configuration
func DefaultPoolConfig() PoolConfig {
	return PoolConfig{
		MaxConnections:   10,
		KeepAliveTime:    30 * time.Second,
		KeepAliveTimeout: 10 * time.Second,
		ConnectTimeout:   10 * time.Second,
	}
}

// NewDialOptions creates gRPC dial options with connection pooling
func NewDialOptions(config PoolConfig) []grpc.DialOption {
	kacp := keepalive.ClientParameters{
		Time:                config.KeepAliveTime,
		Timeout:             config.KeepAliveTimeout,
		PermitWithoutStream: true,
	}

	return []grpc.DialOption{
		grpc.WithKeepaliveParams(kacp),
		grpc.WithDefaultCallOptions(
			grpc.MaxCallRecvMsgSize(100 * 1024 * 1024), // 100MB
			grpc.MaxCallSendMsgSize(100 * 1024 * 1024), // 100MB
		),
	}
}
