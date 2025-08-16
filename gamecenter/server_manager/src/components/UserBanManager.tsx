import React, { useState, useEffect } from 'react';
import {
  Box,
  Card,
  CardContent,
  Typography,
  Button,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
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
} from '@mui/material';
import {
  Add as AddIcon,
  Delete as DeleteIcon,
  Block as BlockIcon,
  Refresh as RefreshIcon,
} from '@mui/icons-material';
import apiService, { UserBan, BanRequest } from '../services/api';

const UserBanManager: React.FC = () => {
  const [bannedUsers, setBannedUsers] = useState<UserBan[]>([]);
  const [loading, setLoading] = useState(true);
  const [openBanDialog, setOpenBanDialog] = useState(false);
  const [snackbar, setSnackbar] = useState<{ open: boolean; message: string; severity: 'success' | 'error' }>({
    open: false,
    message: '',
    severity: 'success',
  });

  // Ban form state
  const [banForm, setBanForm] = useState<BanRequest>({
    user_id: '',
    reason: '',
    duration_hours: 24,
    admin_id: 'admin', // TODO: Get from auth context
  });

  const fetchBannedUsers = async () => {
    try {
      setLoading(true);
      const users = await apiService.getBannedUsers();
      setBannedUsers(users);
    } catch (error) {
      console.error('Failed to fetch banned users:', error);
      showSnackbar('유저 목록을 불러오는데 실패했습니다', 'error');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchBannedUsers();
  }, []);

  const showSnackbar = (message: string, severity: 'success' | 'error') => {
    setSnackbar({ open: true, message, severity });
  };

  const handleBanUser = async () => {
    try {
      await apiService.banUser(banForm);
      showSnackbar('유저가 성공적으로 차단되었습니다', 'success');
      setOpenBanDialog(false);
      setBanForm({
        user_id: '',
        reason: '',
        duration_hours: 24,
        admin_id: 'admin',
      });
      fetchBannedUsers();
    } catch (error) {
      console.error('Failed to ban user:', error);
      showSnackbar('유저 차단에 실패했습니다', 'error');
    }
  };

  const handleUnbanUser = async (userId: string) => {
    try {
      await apiService.unbanUser(userId);
      showSnackbar('유저 차단이 해제되었습니다', 'success');
      fetchBannedUsers();
    } catch (error) {
      console.error('Failed to unban user:', error);
      showSnackbar('차단 해제에 실패했습니다', 'error');
    }
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleString('ko-KR');
  };

  const isExpired = (bannedUntil?: string) => {
    if (!bannedUntil) return false;
    return new Date(bannedUntil) < new Date();
  };

  return (
    <Box p={3}>
      <Box display="flex" justifyContent="space-between" alignItems="center" mb={3}>
        <Typography variant="h4">
          <BlockIcon sx={{ mr: 1, verticalAlign: 'middle' }} />
          유저 차단 관리
        </Typography>
        <Box>
          <IconButton onClick={fetchBannedUsers} color="primary">
            <RefreshIcon />
          </IconButton>
          <Button
            variant="contained"
            startIcon={<AddIcon />}
            onClick={() => setOpenBanDialog(true)}
            sx={{ ml: 1 }}
          >
            유저 차단
          </Button>
        </Box>
      </Box>

      <Card>
        <CardContent>
          <TableContainer component={Paper}>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>유저 ID</TableCell>
                  <TableCell>유저명</TableCell>
                  <TableCell>차단 사유</TableCell>
                  <TableCell>차단 일시</TableCell>
                  <TableCell>해제 예정</TableCell>
                  <TableCell>차단자</TableCell>
                  <TableCell>상태</TableCell>
                  <TableCell>작업</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {loading ? (
                  <TableRow>
                    <TableCell colSpan={8} align="center">
                      로딩 중...
                    </TableCell>
                  </TableRow>
                ) : bannedUsers.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={8} align="center">
                      차단된 유저가 없습니다
                    </TableCell>
                  </TableRow>
                ) : (
                  bannedUsers.map((user) => (
                    <TableRow key={user.user_id}>
                      <TableCell>{user.user_id}</TableCell>
                      <TableCell>{user.username}</TableCell>
                      <TableCell>{user.ban_reason}</TableCell>
                      <TableCell>{formatDate(user.banned_at)}</TableCell>
                      <TableCell>
                        {user.banned_until ? formatDate(user.banned_until) : '영구'}
                      </TableCell>
                      <TableCell>{user.banned_by}</TableCell>
                      <TableCell>
                        {isExpired(user.banned_until) ? (
                          <Chip label="만료됨" color="warning" size="small" />
                        ) : (
                          <Chip label="활성" color="error" size="small" />
                        )}
                      </TableCell>
                      <TableCell>
                        <IconButton
                          onClick={() => handleUnbanUser(user.user_id)}
                          color="primary"
                          size="small"
                        >
                          <DeleteIcon />
                        </IconButton>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </TableContainer>
        </CardContent>
      </Card>

      {/* Ban User Dialog */}
      <Dialog open={openBanDialog} onClose={() => setOpenBanDialog(false)} maxWidth="sm" fullWidth>
        <DialogTitle>유저 차단</DialogTitle>
        <DialogContent>
          <Box sx={{ mt: 2 }}>
            <TextField
              fullWidth
              label="유저 ID"
              value={banForm.user_id}
              onChange={(e) => setBanForm({ ...banForm, user_id: e.target.value })}
              margin="normal"
            />
            <TextField
              fullWidth
              label="차단 사유"
              value={banForm.reason}
              onChange={(e) => setBanForm({ ...banForm, reason: e.target.value })}
              margin="normal"
              multiline
              rows={3}
            />
            <FormControl fullWidth margin="normal">
              <InputLabel>차단 기간</InputLabel>
              <Select
                value={banForm.duration_hours || 0}
                onChange={(e) => setBanForm({ ...banForm, duration_hours: Number(e.target.value) || undefined })}
                label="차단 기간"
              >
                <MenuItem value={1}>1시간</MenuItem>
                <MenuItem value={6}>6시간</MenuItem>
                <MenuItem value={24}>1일</MenuItem>
                <MenuItem value={72}>3일</MenuItem>
                <MenuItem value={168}>1주일</MenuItem>
                <MenuItem value={720}>30일</MenuItem>
                <MenuItem value={0}>영구</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setOpenBanDialog(false)}>취소</Button>
          <Button 
            onClick={handleBanUser} 
            variant="contained" 
            color="error"
            disabled={!banForm.user_id || !banForm.reason}
          >
            차단
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

export default UserBanManager;