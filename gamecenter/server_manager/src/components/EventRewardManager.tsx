import React, { useState, useEffect } from 'react';
import {
  Box,
  Card,
  CardContent,
  Typography,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Select,
  MenuItem,
  FormControl,
  InputLabel,
  IconButton,
  Chip,
  Alert,
  Snackbar,
  LinearProgress,
} from '@mui/material';
import {
  Add as AddIcon,
  CardGiftcard as GiftIcon,
  Stop as StopIcon,
  Refresh as RefreshIcon,
  Timer as TimerIcon,
  People as PeopleIcon,
} from '@mui/icons-material';
import apiService, { EventReward, CreateEventRequest } from '../services/api';

const EventRewardManager: React.FC = () => {
  const [events, setEvents] = useState<EventReward[]>([]);
  const [loading, setLoading] = useState(true);
  const [openCreateDialog, setOpenCreateDialog] = useState(false);
  const [snackbar, setSnackbar] = useState<{ open: boolean; message: string; severity: 'success' | 'error' }>({
    open: false,
    message: '',
    severity: 'success',
  });

  // Event form state
  const [eventForm, setEventForm] = useState<CreateEventRequest>({
    event_name: '',
    reward_type: 'coins',
    reward_amount: 100,
    duration_hours: 24,
  });

  const fetchEvents = async () => {
    try {
      setLoading(true);
      const eventList = await apiService.getEvents();
      setEvents(eventList);
    } catch (error) {
      console.error('Failed to fetch events:', error);
      showSnackbar('이벤트 목록을 불러오는데 실패했습니다', 'error');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchEvents();
    const interval = setInterval(fetchEvents, 10000); // Refresh every 10 seconds
    return () => clearInterval(interval);
  }, []);

  const showSnackbar = (message: string, severity: 'success' | 'error') => {
    setSnackbar({ open: true, message, severity });
  };

  const handleCreateEvent = async () => {
    try {
      await apiService.createEvent(eventForm);
      showSnackbar('이벤트가 성공적으로 생성되었습니다', 'success');
      setOpenCreateDialog(false);
      setEventForm({
        event_name: '',
        reward_type: 'coins',
        reward_amount: 100,
        duration_hours: 24,
      });
      fetchEvents();
    } catch (error) {
      console.error('Failed to create event:', error);
      showSnackbar('이벤트 생성에 실패했습니다', 'error');
    }
  };

  const handleEndEvent = async (eventId: string) => {
    try {
      await apiService.endEvent(eventId);
      showSnackbar('이벤트가 종료되었습니다', 'success');
      fetchEvents();
    } catch (error) {
      console.error('Failed to end event:', error);
      showSnackbar('이벤트 종료에 실패했습니다', 'error');
    }
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleString('ko-KR');
  };

  const calculateProgress = (startTime: string, endTime: string) => {
    const start = new Date(startTime).getTime();
    const end = new Date(endTime).getTime();
    const now = new Date().getTime();
    
    if (now >= end) return 100;
    if (now <= start) return 0;
    
    return ((now - start) / (end - start)) * 100;
  };

  const getTimeRemaining = (endTime: string) => {
    const end = new Date(endTime).getTime();
    const now = new Date().getTime();
    const diff = end - now;
    
    if (diff <= 0) return '종료됨';
    
    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    
    if (hours > 0) return `${hours}시간 ${minutes}분 남음`;
    return `${minutes}분 남음`;
  };

  return (
    <Box p={3}>
      <Box display="flex" justifyContent="space-between" alignItems="center" mb={3}>
        <Typography variant="h4">
          <GiftIcon sx={{ mr: 1, verticalAlign: 'middle' }} />
          이벤트 보상 관리
        </Typography>
        <Box>
          <IconButton onClick={fetchEvents} color="primary">
            <RefreshIcon />
          </IconButton>
          <Button
            variant="contained"
            startIcon={<AddIcon />}
            onClick={() => setOpenCreateDialog(true)}
            sx={{ ml: 1 }}
            color="success"
          >
            새 이벤트 생성
          </Button>
        </Box>
      </Box>

      {loading ? (
        <Box display="flex" justifyContent="center" p={4}>
          <Typography>로딩 중...</Typography>
        </Box>
      ) : events.length === 0 ? (
        <Alert severity="info">진행 중인 이벤트가 없습니다</Alert>
      ) : (
        <Box display="flex" flexWrap="wrap" gap={3}>
          {events.map((event) => (
            <Box key={event.event_id} sx={{ flex: { xs: '1 1 100%', md: '1 1 45%', lg: '1 1 30%' } }}>
              <Card sx={{ height: '100%' }}>
                <CardContent>
                  <Box display="flex" justifyContent="space-between" alignItems="flex-start" mb={2}>
                    <Typography variant="h6" component="div">
                      {event.event_name}
                    </Typography>
                    <Chip
                      label={event.is_active ? '진행 중' : '종료됨'}
                      color={event.is_active ? 'success' : 'default'}
                      size="small"
                    />
                  </Box>

                  <Box mb={2}>
                    <Typography variant="body2" color="text.secondary" gutterBottom>
                      보상: {event.reward_amount} {event.reward_type}
                    </Typography>
                  </Box>

                  <Box mb={2}>
                    <Box display="flex" alignItems="center" mb={1}>
                      <PeopleIcon fontSize="small" sx={{ mr: 1 }} />
                      <Typography variant="body2">
                        참여자: {event.participants_count}명
                      </Typography>
                    </Box>
                    
                    <Box display="flex" alignItems="center">
                      <TimerIcon fontSize="small" sx={{ mr: 1 }} />
                      <Typography variant="body2">
                        {event.is_active ? getTimeRemaining(event.end_time) : '종료됨'}
                      </Typography>
                    </Box>
                  </Box>

                  {event.is_active && (
                    <Box mb={2}>
                      <Typography variant="caption" color="text.secondary">
                        진행률
                      </Typography>
                      <LinearProgress
                        variant="determinate"
                        value={calculateProgress(event.start_time, event.end_time)}
                        sx={{ mt: 1 }}
                      />
                    </Box>
                  )}

                  <Box mt={2}>
                    <Typography variant="caption" display="block" color="text.secondary">
                      시작: {formatDate(event.start_time)}
                    </Typography>
                    <Typography variant="caption" display="block" color="text.secondary">
                      종료: {formatDate(event.end_time)}
                    </Typography>
                  </Box>

                  {event.is_active && (
                    <Box mt={2}>
                      <Button
                        fullWidth
                        variant="outlined"
                        color="error"
                        startIcon={<StopIcon />}
                        onClick={() => handleEndEvent(event.event_id)}
                      >
                        이벤트 종료
                      </Button>
                    </Box>
                  )}
                </CardContent>
              </Card>
            </Box>
          ))}
        </Box>
      )}

      {/* Create Event Dialog */}
      <Dialog open={openCreateDialog} onClose={() => setOpenCreateDialog(false)} maxWidth="sm" fullWidth>
        <DialogTitle>새 이벤트 생성</DialogTitle>
        <DialogContent>
          <Box sx={{ mt: 2 }}>
            <TextField
              fullWidth
              label="이벤트 이름"
              value={eventForm.event_name}
              onChange={(e) => setEventForm({ ...eventForm, event_name: e.target.value })}
              margin="normal"
            />
            
            <FormControl fullWidth margin="normal">
              <InputLabel>보상 유형</InputLabel>
              <Select
                value={eventForm.reward_type}
                onChange={(e) => setEventForm({ ...eventForm, reward_type: e.target.value })}
                label="보상 유형"
              >
                <MenuItem value="coins">코인</MenuItem>
                <MenuItem value="gems">보석</MenuItem>
                <MenuItem value="items">아이템</MenuItem>
                <MenuItem value="exp">경험치</MenuItem>
              </Select>
            </FormControl>
            
            <TextField
              fullWidth
              type="number"
              label="보상 수량"
              value={eventForm.reward_amount}
              onChange={(e) => setEventForm({ ...eventForm, reward_amount: Number(e.target.value) })}
              margin="normal"
            />
            
            <FormControl fullWidth margin="normal">
              <InputLabel>이벤트 기간</InputLabel>
              <Select
                value={eventForm.duration_hours}
                onChange={(e) => setEventForm({ ...eventForm, duration_hours: Number(e.target.value) })}
                label="이벤트 기간"
              >
                <MenuItem value={1}>1시간</MenuItem>
                <MenuItem value={6}>6시간</MenuItem>
                <MenuItem value={12}>12시간</MenuItem>
                <MenuItem value={24}>1일</MenuItem>
                <MenuItem value={72}>3일</MenuItem>
                <MenuItem value={168}>1주일</MenuItem>
                <MenuItem value={336}>2주일</MenuItem>
                <MenuItem value={720}>30일</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setOpenCreateDialog(false)}>취소</Button>
          <Button 
            onClick={handleCreateEvent} 
            variant="contained" 
            color="success"
            disabled={!eventForm.event_name}
          >
            생성
          </Button>
        </DialogActions>
      </Dialog>

      {/* Snackbar for notifications */}
      <Snackbar
        open={snackbar.open}
        autoHideDuration={6000}
        onClose={() => setSnackbar({ ...snackbar, open: false })}
      >
        <Alert 
          onClose={() => setSnackbar({ ...snackbar, open: false })} 
          severity={snackbar.severity}
        >
          {snackbar.message}
        </Alert>
      </Snackbar>
    </Box>
  );
};

export default EventRewardManager;