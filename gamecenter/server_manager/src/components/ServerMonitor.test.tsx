import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import ServerMonitor from './ServerMonitor';
import apiService from '../services/api';

// Mock the API service
jest.mock('../services/api');

const mockServerStatus = {
  server_name: 'GameCenter Unified Server',
  is_running: true,
  uptime_seconds: 3600,
  connected_clients: 150,
  memory_usage_mb: 256.5,
  cpu_usage: {
    timestamp: '2024-01-01T12:00:00Z',
    total_usage: 45.5,
    core_usage: [40.0, 50.0, 45.0, 46.0],
    process_usage: 15.5,
  },
};

const mockAllServersStatus = [
  {
    server_name: 'TCP Server',
    is_running: true,
    uptime_seconds: 3600,
    connected_clients: 100,
    memory_usage_mb: 128.5,
    cpu_usage: {
      timestamp: '2024-01-01T12:00:00Z',
      total_usage: 25.5,
      core_usage: [],
      process_usage: 10.5,
    },
  },
  {
    server_name: 'gRPC Server',
    is_running: true,
    uptime_seconds: 3600,
    connected_clients: 50,
    memory_usage_mb: 64.5,
    cpu_usage: {
      timestamp: '2024-01-01T12:00:00Z',
      total_usage: 15.5,
      core_usage: [],
      process_usage: 5.5,
    },
  },
];

describe('ServerMonitor Component', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders loading state initially', () => {
    render(<ServerMonitor />);
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it('displays server status after loading', async () => {
    (apiService.getServerStatus as jest.Mock).mockResolvedValue(mockServerStatus);
    (apiService.getAllServersStatus as jest.Mock).mockResolvedValue(mockAllServersStatus);

    render(<ServerMonitor />);

    await waitFor(() => {
      expect(screen.getByText('GameCenter Unified Server')).toBeInTheDocument();
    });

    expect(screen.getByText(/150.*clients/i)).toBeInTheDocument();
    expect(screen.getByText(/256\.5.*MB/i)).toBeInTheDocument();
    expect(screen.getByText(/45\.5.*%/i)).toBeInTheDocument();
  });

  it('displays all servers status', async () => {
    (apiService.getServerStatus as jest.Mock).mockResolvedValue(mockServerStatus);
    (apiService.getAllServersStatus as jest.Mock).mockResolvedValue(mockAllServersStatus);

    render(<ServerMonitor />);

    await waitFor(() => {
      expect(screen.getByText('TCP Server')).toBeInTheDocument();
      expect(screen.getByText('gRPC Server')).toBeInTheDocument();
    });
  });

  it('handles error state gracefully', async () => {
    (apiService.getServerStatus as jest.Mock).mockRejectedValue(new Error('API Error'));
    (apiService.getAllServersStatus as jest.Mock).mockRejectedValue(new Error('API Error'));

    render(<ServerMonitor />);

    await waitFor(() => {
      expect(screen.getByText(/error loading server status/i)).toBeInTheDocument();
    });
  });

  it('refreshes data periodically', async () => {
    jest.useFakeTimers();
    (apiService.getServerStatus as jest.Mock).mockResolvedValue(mockServerStatus);
    (apiService.getAllServersStatus as jest.Mock).mockResolvedValue(mockAllServersStatus);

    render(<ServerMonitor />);

    await waitFor(() => {
      expect(apiService.getServerStatus).toHaveBeenCalledTimes(1);
    });

    // Fast-forward 5 seconds (refresh interval)
    jest.advanceTimersByTime(5000);

    await waitFor(() => {
      expect(apiService.getServerStatus).toHaveBeenCalledTimes(2);
    });

    jest.useRealTimers();
  });
});