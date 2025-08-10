#!/usr/bin/env python3
"""
간단한 UDP 서버 - 부하 테스트용
"""

import socket
import struct
import json
import time
import threading
from datetime import datetime

class SimpleUDPServer:
    def __init__(self, host='127.0.0.1', port=5000):
        self.host = host
        self.port = port
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.socket.bind((host, port))
        self.running = True
        self.stats = {
            'messages_received': 0,
            'bytes_received': 0,
            'start_time': time.time(),
            'clients': set()
        }
        
    def run(self):
        print(f"[UDP Server] Started on {self.host}:{self.port}")
        print("Waiting for clients...")
        
        # 통계 출력 스레드
        stats_thread = threading.Thread(target=self.print_stats)
        stats_thread.daemon = True
        stats_thread.start()
        
        try:
            while self.running:
                try:
                    data, addr = self.socket.recvfrom(4096)
                    
                    # 통계 업데이트
                    self.stats['messages_received'] += 1
                    self.stats['bytes_received'] += len(data)
                    self.stats['clients'].add(addr)
                    
                    # 에코 응답 (선택적)
                    # self.socket.sendto(data, addr)
                    
                except socket.timeout:
                    continue
                except Exception as e:
                    print(f"에러: {e}")
                    
        except KeyboardInterrupt:
            print("\n서버 종료...")
        finally:
            self.socket.close()
            
    def print_stats(self):
        """주기적으로 통계 출력"""
        while self.running:
            time.sleep(5)
            elapsed = time.time() - self.stats['start_time']
            msg_per_sec = self.stats['messages_received'] / elapsed if elapsed > 0 else 0
            
            print(f"\n[STATS] {datetime.now().strftime('%H:%M:%S')}")
            print(f"  - Messages received: {self.stats['messages_received']:,}")
            print(f"  - Messages/sec: {msg_per_sec:.1f}")
            print(f"  - Active clients: {len(self.stats['clients'])}")
            print(f"  - Data received: {self.stats['bytes_received'] / 1024**2:.2f} MB")

if __name__ == "__main__":
    server = SimpleUDPServer()
    server.run()