import React, { useState, useEffect } from 'react';
import {
  Box,
  Card,
  CardContent,
  Typography,
  LinearProgress,
  Chip,
  Alert,
  CircularProgress,
  Paper,
} from '@mui/material';
import {
  Computer as ComputerIcon,
  Memory as MemoryIcon,
  People as PeopleIcon,
  Timer as TimerIcon,
} from '@mui/icons-material';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';
import apiService, { ServerStatus } from '../services/api';

const ServerMonitor: React.FC = () => {
  const [mainServerStatus, setMainServerStatus] = useState<ServerStatus | null>(null);
  const [allServersStatus, setAllServersStatus] = useState<ServerStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [cpuHistory, setCpuHistory] = useState<any[]>([]);

  const fetchServerStatus = async () => {
    try {
      const [mainStatus, allStatus] = await Promise.all([
        apiService.getServerStatus(),
        apiService.getAllServersStatus(),
      ]);
      
      setMainServerStatus(mainStatus);
      setAllServersStatus(allStatus);
      
      // Add to CPU history for chart
      setCpuHistory(prev => {
        const newEntry = {
          time: new Date().toLocaleTimeString(),
          cpu: mainStatus.cpu_usage.total_usage,
          memory: (mainStatus.memory_usage_mb / 1024).toFixed(2),
        };
        const updated = [...prev, newEntry];
        return updated.slice(-20); // Keep last 20 entries
      });
      
      setError(null);
    } catch (err) {
      setError('Error loading server status');
      console.error('Failed to fetch server status:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchServerStatus();
    const interval = setInterval(fetchServerStatus, 5000); // Refresh every 5 seconds
    return () => clearInterval(interval);
  }, []);

  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
  };

  if (loading) {
    return (
      <Box display="flex" justifyContent="center" alignItems="center" minHeight="400px">
        <CircularProgress />
        <Typography ml={2}>Loading server status...</Typography>
      </Box>
    );
  }

  if (error) {
    return (
      <Alert severity="error" sx={{ m: 2 }}>
        {error}
      </Alert>
    );
  }

  return (
    <Box p={3}>
      <Typography variant="h4" gutterBottom>
        서버 모니터링 대시보드
      </Typography>

      {/* Main Server Status */}
      {mainServerStatus && (
        <Card sx={{ mb: 3 }}>
          <CardContent>
            <Box display="flex" alignItems="center" mb={2}>
              <ComputerIcon sx={{ mr: 1 }} />
              <Typography variant="h5">{mainServerStatus.server_name}</Typography>
              <Chip
                label={mainServerStatus.is_running ? '실행 중' : '중지됨'}
                color={mainServerStatus.is_running ? 'success' : 'error'}
                size="small"
                sx={{ ml: 2 }}
              />
            </Box>

            <Box display="flex" flexWrap="wrap" gap={3}>
              <Box flex={{ xs: '1 1 100%', sm: '1 1 45%', md: '1 1 22%' }}>
                <Paper sx={{ p: 2, textAlign: 'center' }}>
                  <TimerIcon color="primary" />
                  <Typography variant="h6">
                    {formatUptime(mainServerStatus.uptime_seconds)}
                  </Typography>
                  <Typography variant="caption">가동 시간</Typography>
                </Paper>
              </Box>

              <Box flex={{ xs: '1 1 100%', sm: '1 1 45%', md: '1 1 22%' }}>
                <Paper sx={{ p: 2, textAlign: 'center' }}>
                  <PeopleIcon color="primary" />
                  <Typography variant="h6">
                    {mainServerStatus.connected_clients} clients
                  </Typography>
                  <Typography variant="caption">연결된 클라이언트</Typography>
                </Paper>
              </Box>

              <Box flex={{ xs: '1 1 100%', sm: '1 1 45%', md: '1 1 22%' }}>
                <Paper sx={{ p: 2, textAlign: 'center' }}>
                  <MemoryIcon color="primary" />
                  <Typography variant="h6">
                    {mainServerStatus.memory_usage_mb.toFixed(1)} MB
                  </Typography>
                  <Typography variant="caption">메모리 사용량</Typography>
                </Paper>
              </Box>

              <Box flex={{ xs: '1 1 100%', sm: '1 1 45%', md: '1 1 22%' }}>
                <Paper sx={{ p: 2, textAlign: 'center' }}>
                  <Typography variant="h6" color={
                    mainServerStatus.cpu_usage.total_usage > 80 ? 'error' : 
                    mainServerStatus.cpu_usage.total_usage > 50 ? 'warning.main' : 'success.main'
                  }>
                    {mainServerStatus.cpu_usage.total_usage.toFixed(1)}%
                  </Typography>
                  <Typography variant="caption">CPU 사용률</Typography>
                  <LinearProgress 
                    variant="determinate" 
                    value={mainServerStatus.cpu_usage.total_usage}
                    color={
                      mainServerStatus.cpu_usage.total_usage > 80 ? 'error' : 
                      mainServerStatus.cpu_usage.total_usage > 50 ? 'warning' : 'success'
                    }
                    sx={{ mt: 1 }}
                  />
                </Paper>
              </Box>
            </Box>

            {/* CPU Usage Chart */}
            {cpuHistory.length > 0 && (
              <Box mt={3}>
                <Typography variant="h6" gutterBottom>
                  실시간 리소스 사용량
                </Typography>
                <ResponsiveContainer width="100%" height={300}>
                  <LineChart data={cpuHistory}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="time" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    <Line 
                      type="monotone" 
                      dataKey="cpu" 
                      stroke="#8884d8" 
                      name="CPU (%)"
                      strokeWidth={2}
                    />
                    <Line 
                      type="monotone" 
                      dataKey="memory" 
                      stroke="#82ca9d" 
                      name="Memory (GB)"
                      strokeWidth={2}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </Box>
            )}
          </CardContent>
        </Card>
      )}

      {/* Individual Servers Status */}
      <Typography variant="h5" gutterBottom>
        개별 서버 상태
      </Typography>
      <Box display="flex" flexWrap="wrap" gap={2}>
        {allServersStatus.map((server) => (
          <Box key={server.server_name} sx={{ flex: { xs: '1 1 100%', md: '1 1 30%' } }}>
            <Card>
              <CardContent>
                <Box display="flex" alignItems="center" justifyContent="space-between" mb={2}>
                  <Typography variant="h6">{server.server_name}</Typography>
                  <Chip
                    label={server.is_running ? '실행 중' : '중지됨'}
                    color={server.is_running ? 'success' : 'error'}
                    size="small"
                  />
                </Box>
                
                <Box mb={1}>
                  <Typography variant="body2" color="textSecondary">
                    연결: {server.connected_clients} 클라이언트
                  </Typography>
                </Box>
                
                <Box mb={1}>
                  <Typography variant="body2" color="textSecondary">
                    메모리: {server.memory_usage_mb.toFixed(1)} MB
                  </Typography>
                </Box>
                
                <Box>
                  <Typography variant="body2" color="textSecondary">
                    CPU: {server.cpu_usage.process_usage.toFixed(1)}%
                  </Typography>
                  <LinearProgress 
                    variant="determinate" 
                    value={server.cpu_usage.process_usage}
                    color={
                      server.cpu_usage.process_usage > 80 ? 'error' : 
                      server.cpu_usage.process_usage > 50 ? 'warning' : 'success'
                    }
                  />
                </Box>
              </CardContent>
            </Card>
          </Box>
        ))}
      </Box>
    </Box>
  );
};

export default ServerMonitor;