import axios from 'axios';

const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:8080/api/admin';

const api = axios.create({
  baseURL: API_BASE_URL,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Types
export interface CpuUsage {
  timestamp: string;
  total_usage: number;
  core_usage: number[];
  process_usage: number;
}

export interface ServerStatus {
  server_name: string;
  is_running: boolean;
  uptime_seconds: number;
  connected_clients: number;
  memory_usage_mb: number;
  cpu_usage: CpuUsage;
}

export interface UserBan {
  user_id: string;
  username: string;
  ban_reason: string;
  banned_at: string;
  banned_until?: string;
  banned_by: string;
}

export interface EventReward {
  event_id: string;
  event_name: string;
  reward_type: string;
  reward_amount: number;
  start_time: string;
  end_time: string;
  is_active: boolean;
  participants_count: number;
}

export interface BanRequest {
  user_id: string;
  reason: string;
  duration_hours?: number;
  admin_id: string;
}

export interface CreateEventRequest {
  event_name: string;
  reward_type: string;
  reward_amount: number;
  duration_hours: number;
}

// API Service
class ApiService {
  // Server Status APIs
  async getServerStatus(): Promise<ServerStatus> {
    const response = await api.get<ServerStatus>('/status');
    return response.data;
  }

  async getAllServersStatus(): Promise<ServerStatus[]> {
    const response = await api.get<ServerStatus[]>('/servers');
    return response.data;
  }

  // User Ban APIs
  async getBannedUsers(): Promise<UserBan[]> {
    const response = await api.get<UserBan[]>('/users/banned');
    return response.data;
  }

  async banUser(request: BanRequest): Promise<UserBan> {
    const response = await api.post<UserBan>('/users/ban', request);
    return response.data;
  }

  async unbanUser(userId: string): Promise<{ message: string }> {
    const response = await api.delete<{ message: string }>(`/users/unban/${userId}`);
    return response.data;
  }

  // Event APIs
  async getEvents(): Promise<EventReward[]> {
    const response = await api.get<EventReward[]>('/events');
    return response.data;
  }

  async createEvent(request: CreateEventRequest): Promise<EventReward> {
    const response = await api.post<EventReward>('/events', request);
    return response.data;
  }

  async endEvent(eventId: string): Promise<EventReward> {
    const response = await api.put<EventReward>(`/events/${eventId}/end`);
    return response.data;
  }
}

export default new ApiService();