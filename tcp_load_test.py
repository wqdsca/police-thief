#!/usr/bin/env python3
"""
TCP Server Load Test
1vCPU 1GB RAM 환경에서 100개 방, 각 방당 10명(총 1000명)의 실시간 이동 테스트
"""

import asyncio
import socket
import struct
import json
import time
import random
import psutil
import os
import sys
from datetime import datetime
from typing import Dict, List, Tuple
from dataclasses import dataclass, field
from colorama import init, Fore, Style
import threading

# Colorama 초기화
init(autoreset=True)

# 서버 설정
SERVER_HOST = "127.0.0.1"
SERVER_PORT = 4000
NUM_ROOMS = 100
PLAYERS_PER_ROOM = 10
TOTAL_PLAYERS = NUM_ROOMS * PLAYERS_PER_ROOM
UPDATE_INTERVAL = 0.1  # 100ms
TEST_DURATION = 30  # 30초
WORLD_SIZE = 100.0  # 게임 월드 크기

@dataclass
class PerformanceMetrics:
    """성능 메트릭 데이터"""
    messages_sent: int = 0
    messages_received: int = 0
    bytes_sent: int = 0
    bytes_received: int = 0
    errors: int = 0
    connections: int = 0
    connection_errors: int = 0
    latencies: List[float] = field(default_factory=list)
    start_time: float = field(default_factory=time.time)
    
    @property
    def avg_latency(self) -> float:
        return sum(self.latencies) / len(self.latencies) if self.latencies else 0
    
    @property
    def max_latency(self) -> float:
        return max(self.latencies) if self.latencies else 0
    
    @property
    def min_latency(self) -> float:
        return min(self.latencies) if self.latencies else 0
    
    @property
    def messages_per_second(self) -> float:
        elapsed = time.time() - self.start_time
        return self.messages_sent / elapsed if elapsed > 0 else 0
    
    @property
    def throughput_mbps(self) -> float:
        elapsed = time.time() - self.start_time
        total_bytes = self.bytes_sent + self.bytes_received
        return (total_bytes * 8 / 1_000_000) / elapsed if elapsed > 0 else 0

class VirtualPlayer:
    """가상 플레이어 클라이언트"""
    
    def __init__(self, player_id: int, room_id: int, metrics: PerformanceMetrics):
        self.player_id = player_id
        self.room_id = room_id
        self.metrics = metrics
        self.reader = None
        self.writer = None
        self.position = [
            random.uniform(0, WORLD_SIZE),
            random.uniform(0, WORLD_SIZE),
            0.0
        ]
        self.velocity = [
            random.uniform(-1, 1),
            random.uniform(-1, 1),
            0
        ]
        self.is_running = True
        self.connected = False
        
    async def connect(self):
        """TCP 서버에 연결"""
        try:
            self.reader, self.writer = await asyncio.open_connection(
                SERVER_HOST, SERVER_PORT
            )
            self.connected = True
            self.metrics.connections += 1
            
            # 연결 메시지 전송
            message = {
                "type": "connect",
                "player_id": self.player_id,
                "room_id": self.room_id,
                "nickname": f"Player{self.player_id}",
                "timestamp": time.time()
            }
            await self.send_message(message)
            return True
        except Exception as e:
            print(f"{Fore.RED}Player {self.player_id} 연결 실패: {e}")
            self.metrics.connection_errors += 1
            return False
    
    async def send_message(self, message: dict):
        """메시지 전송 (4바이트 길이 헤더 + JSON)"""
        try:
            if not self.writer or not self.connected:
                return
                
            data = json.dumps(message).encode('utf-8')
            # 4바이트 길이 헤더 + 데이터
            packet = struct.pack('!I', len(data)) + data
            
            self.writer.write(packet)
            await self.writer.drain()
            
            self.metrics.messages_sent += 1
            self.metrics.bytes_sent += len(packet)
        except Exception as e:
            self.metrics.errors += 1
            
    async def read_messages(self):
        """서버로부터 메시지 수신 (백그라운드)"""
        try:
            while self.is_running and self.connected:
                if not self.reader:
                    break
                    
                # 4바이트 길이 헤더 읽기
                length_data = await self.reader.read(4)
                if len(length_data) != 4:
                    break
                    
                length = struct.unpack('!I', length_data)[0]
                if length > 1024 * 1024:  # 1MB 제한
                    break
                    
                # 실제 메시지 데이터 읽기
                message_data = await self.reader.read(length)
                if len(message_data) != length:
                    break
                    
                self.metrics.messages_received += 1
                self.metrics.bytes_received += len(length_data) + len(message_data)
                
        except asyncio.CancelledError:
            pass
        except Exception as e:
            self.metrics.errors += 1
            
    async def update_position(self):
        """위치 업데이트"""
        # 간단한 물리 시뮬레이션
        self.position[0] += self.velocity[0]
        self.position[1] += self.velocity[1]
        
        # 월드 경계 체크
        for i in range(2):
            if self.position[i] < 0 or self.position[i] > WORLD_SIZE:
                self.velocity[i] *= -1
                self.position[i] = max(0, min(WORLD_SIZE, self.position[i]))
        
        # 랜덤 방향 변경
        if random.random() < 0.1:
            self.velocity[0] = random.uniform(-1, 1)
            self.velocity[1] = random.uniform(-1, 1)
        
        message = {
            "type": "move",
            "player_id": self.player_id,
            "room_id": self.room_id,
            "x": round(self.position[0], 2),
            "y": round(self.position[1], 2),
            "z": round(self.position[2], 2),
            "timestamp": time.time()
        }
        
        start_time = time.time()
        await self.send_message(message)
        latency = (time.time() - start_time) * 1000  # ms
        self.metrics.latencies.append(latency)
        
    async def run(self):
        """플레이어 실행"""
        if not await self.connect():
            return
            
        try:
            # 메시지 수신 태스크 시작
            read_task = asyncio.create_task(self.read_messages())
            
            while self.is_running:
                await self.update_position()
                await asyncio.sleep(UPDATE_INTERVAL)
                
            read_task.cancel()
            
        except Exception as e:
            print(f"{Fore.RED}Player {self.player_id} 실행 오류: {e}")
            self.metrics.errors += 1
        finally:
            await self.disconnect()
            
    async def disconnect(self):
        """연결 종료"""
        if self.writer and self.connected:
            try:
                message = {
                    "type": "disconnect",
                    "player_id": self.player_id,
                    "room_id": self.room_id,
                    "timestamp": time.time()
                }
                await self.send_message(message)
                
                self.writer.close()
                await self.writer.wait_closed()
                self.connected = False
                
            except Exception:
                pass
    
    def stop(self):
        """플레이어 중지"""
        self.is_running = False

class LoadTestManager:
    """부하 테스트 관리자"""
    
    def __init__(self):
        self.metrics = PerformanceMetrics()
        self.players: List[VirtualPlayer] = []
        self.is_running = True
        
    def print_header(self):
        """헤더 출력"""
        print(f"\n{Fore.CYAN}{'='*60}")
        print(f"{Fore.YELLOW}TCP Server Load Test")
        print(f"{Fore.CYAN}{'='*60}")
        print(f"테스트 환경: 1vCPU, 1GB RAM 시뮬레이션")
        print(f"방 개수: {NUM_ROOMS}")
        print(f"방당 플레이어: {PLAYERS_PER_ROOM}")
        print(f"총 플레이어: {TOTAL_PLAYERS}")
        print(f"업데이트 주기: {UPDATE_INTERVAL*1000:.0f}ms")
        print(f"테스트 시간: {TEST_DURATION}초")
        print(f"{Fore.CYAN}{'='*60}\n")
        
    async def create_players(self):
        """플레이어 생성"""
        print(f"{Fore.GREEN}플레이어 생성 중...")
        
        for room_id in range(NUM_ROOMS):
            for player_idx in range(PLAYERS_PER_ROOM):
                player_id = room_id * PLAYERS_PER_ROOM + player_idx
                player = VirtualPlayer(player_id, room_id, self.metrics)
                self.players.append(player)
                
            if (room_id + 1) % 10 == 0:
                print(f"진행상황: {room_id + 1}/{NUM_ROOMS} 방 완료")
                
        print(f"{Fore.GREEN}총 {len(self.players)}명 플레이어 생성 완료\n")
        
    async def run_test(self):
        """테스트 실행"""
        self.print_header()
        await self.create_players()
        
        print(f"{Fore.YELLOW}테스트 시작!")
        print(f"{Fore.CYAN}연결 설정 중...\n")
        
        # 배치별 연결 (서버 과부하 방지)
        tasks = []
        batch_size = 50  # 동시 연결 제한
        
        for i in range(0, len(self.players), batch_size):
            batch = self.players[i:i+batch_size]
            for player in batch:
                task = asyncio.create_task(player.run())
                tasks.append(task)
            await asyncio.sleep(0.1)  # 배치 간격
            
        # 모니터링 시작
        monitor_task = asyncio.create_task(self.monitor_stats())
        
        # 테스트 시간 대기
        await asyncio.sleep(TEST_DURATION)
        
        print(f"\n{Fore.YELLOW}테스트 종료 중...")
        self.is_running = False
        
        # 모든 플레이어 종료
        for player in self.players:
            player.stop()
            
        # 태스크 정리
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        monitor_task.cancel()
        
        # 결과 출력
        self.print_results()
        
    async def monitor_stats(self):
        """실시간 통계 출력"""
        while self.is_running:
            await asyncio.sleep(2)
            self.print_live_stats()
            
    def print_live_stats(self):
        """실시간 통계"""
        elapsed = time.time() - self.metrics.start_time
        
        # 커서를 5줄 위로 이동 (이전 출력 덮어쓰기)
        sys.stdout.write('\033[5A')
        sys.stdout.flush()
        
        print(f"실시간 현황 [{elapsed:.1f}초 경과]")
        print(f"연결된 플레이어: {self.metrics.connections} (에러: {self.metrics.connection_errors})")
        print(f"전송 메시지: {self.metrics.messages_sent:,} msgs ({self.metrics.messages_per_second:.1f} msg/s)")
        print(f"수신 메시지: {self.metrics.messages_received:,}")
        print(f"네트워크 처리량: {self.metrics.throughput_mbps:.2f} Mbps")
        
    def print_results(self):
        """최종 결과 출력"""
        elapsed = time.time() - self.metrics.start_time
        
        print(f"\n{Fore.CYAN}{'='*60}")
        print(f"{Fore.YELLOW}테스트 결과")
        print(f"{Fore.CYAN}{'='*60}")
        
        print(f"\n{Fore.GREEN}[기본 정보]")
        print(f"테스트 시간: {elapsed:.2f}초")
        print(f"목표 방 수: {NUM_ROOMS}")
        print(f"목표 플레이어: {TOTAL_PLAYERS}")
        print(f"업데이트 주기: {UPDATE_INTERVAL*1000:.0f}ms")
        
        print(f"\n{Fore.GREEN}[연결 성능]")
        print(f"성공 연결: {self.metrics.connections}")
        print(f"연결 에러: {self.metrics.connection_errors}")
        print(f"연결 성공률: {(self.metrics.connections/(self.metrics.connections+max(1,self.metrics.connection_errors))) * 100:.2f}%")
        
        print(f"\n{Fore.GREEN}[메시지 성능]")
        print(f"전송 메시지: {self.metrics.messages_sent:,}")
        print(f"수신 메시지: {self.metrics.messages_received:,}")
        print(f"메시지/초: {self.metrics.messages_per_second:.2f}")
        print(f"처리량: {self.metrics.throughput_mbps:.2f} Mbps")
        print(f"전송 데이터: {self.metrics.bytes_sent:,} 바이트")
        print(f"수신 데이터: {self.metrics.bytes_received:,} 바이트")
        
        print(f"\n{Fore.GREEN}[지연시간]")
        print(f"평균 지연시간: {self.metrics.avg_latency:.2f}ms")
        print(f"최소 지연시간: {self.metrics.min_latency:.2f}ms")
        print(f"최대 지연시간: {self.metrics.max_latency:.2f}ms")
        
        print(f"\n{Fore.GREEN}[에러 현황]")
        print(f"총 에러 수: {self.metrics.errors}")
        print(f"에러 비율: {(self.metrics.errors/max(1, self.metrics.messages_sent)) * 100:.3f}%")
        
        # 시스템 리소스 정보
        self.print_system_resources()
        
        print(f"\n{Fore.CYAN}{'='*60}")
        
        # 성능 평가
        self.evaluate_performance()
        
    def print_system_resources(self):
        """시스템 리소스 정보"""
        print(f"\n{Fore.GREEN}[시스템 리소스]")
        
        # CPU 사용률
        cpu_percent = psutil.cpu_percent(interval=1)
        print(f"현재 CPU 사용률: {cpu_percent:.1f}%")
        
        # 메모리 사용량
        memory = psutil.virtual_memory()
        print(f"현재 메모리 사용량: {memory.used / 1024**3:.2f}GB / {memory.total / 1024**3:.2f}GB ({memory.percent:.1f}%)")
        
        # TCP 서버 프로세스 (있다면)
        try:
            for proc in psutil.process_iter(['pid', 'name', 'cpu_percent', 'memory_info']):
                if 'tcpserver' in proc.info['name'].lower():
                    mem_mb = proc.info['memory_info'].rss / 1024**2
                    print(f"TCP 서버 프로세스: CPU {proc.info['cpu_percent']:.1f}%, 메모리 {mem_mb:.1f}MB")
        except:
            pass
            
    def evaluate_performance(self):
        """성능 평가"""
        print(f"\n{Fore.YELLOW}[성능 평가]")
        
        msg_per_sec = self.metrics.messages_per_second
        avg_latency = self.metrics.avg_latency
        error_rate = self.metrics.errors / max(1, self.metrics.messages_sent)
        connection_rate = self.metrics.connections / max(1, TOTAL_PLAYERS)
        
        score = 0
        max_score = 0
        
        # 메시지 처리량 평가 (목표: 5,000 msg/s)
        if msg_per_sec >= 5000:
            score += 25
            eval_msg = f"{Fore.GREEN}우수"
        elif msg_per_sec >= 2000:
            score += 20
            eval_msg = f"{Fore.YELLOW}양호"
        else:
            score += 10
            eval_msg = f"{Fore.RED}개선 필요"
        max_score += 25
        print(f"메시지 처리량: {eval_msg} ({msg_per_sec:.0f} msg/s)")
        
        # 지연시간 평가 (목표: <100ms)
        if avg_latency <= 50:
            score += 25
            eval_lat = f"{Fore.GREEN}우수"
        elif avg_latency <= 100:
            score += 20
            eval_lat = f"{Fore.YELLOW}양호"
        else:
            score += 10
            eval_lat = f"{Fore.RED}개선 필요"
        max_score += 25
        print(f"지연시간: {eval_lat} ({avg_latency:.1f}ms)")
        
        # 연결 성공률 평가
        if connection_rate >= 0.9:
            score += 25
            eval_conn = f"{Fore.GREEN}우수"
        elif connection_rate >= 0.7:
            score += 20
            eval_conn = f"{Fore.YELLOW}양호"
        else:
            score += 10
            eval_conn = f"{Fore.RED}개선 필요"
        max_score += 25
        print(f"연결 성공률: {eval_conn} ({connection_rate*100:.1f}%)")
        
        # 에러율 평가 (목표: <0.1%)
        if error_rate < 0.001:
            score += 25
            eval_err = f"{Fore.GREEN}우수"
        elif error_rate < 0.01:
            score += 20
            eval_err = f"{Fore.YELLOW}양호"
        else:
            score += 10
            eval_err = f"{Fore.RED}개선 필요"
        max_score += 25
        print(f"에러율: {eval_err} ({error_rate*100:.3f}%)")
        
        # 총점 계산
        total_score = (score / max_score) * 100
        if total_score >= 80:
            grade = f"{Fore.GREEN}A"
            comment = "운영 환경 준비 완료!"
        elif total_score >= 60:
            grade = f"{Fore.YELLOW}B"
            comment = "추가 최적화 권장"
        else:
            grade = f"{Fore.RED}C"
            comment = "성능 개선 필요"
            
        print(f"\n종합 점수: {Fore.CYAN}{total_score:.0f}/100 (등급: {grade})")
        print(f"종합 평가: {Fore.CYAN}{comment}")

async def main():
    """메인 함수"""
    manager = LoadTestManager()
    
    try:
        await manager.run_test()
    except KeyboardInterrupt:
        print(f"\n{Fore.YELLOW}사용자에 의해 중단됨.")
    except Exception as e:
        print(f"\n{Fore.RED}테스트 실행 오류: {e}")

if __name__ == "__main__":
    # Windows 이벤트 루프 정책
    if sys.platform == 'win32':
        asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())
    
    asyncio.run(main())