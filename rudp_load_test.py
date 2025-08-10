#!/usr/bin/env python3
"""
RUDP    
1vCPU 1GB RAM  100 ,   10( 1000)   

:
    pip install asyncio aiofiles psutil colorama
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

# Colorama 
init(autoreset=True)

#  
SERVER_HOST = "127.0.0.1"
SERVER_PORT = 5000
NUM_ROOMS = 100
PLAYERS_PER_ROOM = 10
TOTAL_PLAYERS = NUM_ROOMS * PLAYERS_PER_ROOM
UPDATE_INTERVAL = 0.1  # 100ms
TEST_DURATION = 30  # 30
WORLD_SIZE = 100.0  #   

@dataclass
class PerformanceMetrics:
    """  """
    messages_sent: int = 0
    messages_received: int = 0
    bytes_sent: int = 0
    bytes_received: int = 0
    errors: int = 0
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
    """  """
    
    def __init__(self, player_id: int, room_id: int, metrics: PerformanceMetrics):
        self.player_id = player_id
        self.room_id = room_id
        self.metrics = metrics
        self.socket = None
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
        
    async def connect(self):
        """ """
        try:
            self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            self.socket.settimeout(1.0)
            
            #   
            message = {
                "type": "connect",
                "player_id": self.player_id,
                "room_id": self.room_id,
                "timestamp": time.time()
            }
            await self.send_message(message)
            return True
        except Exception as e:
            print(f"{Fore.RED}Player {self.player_id}  : {e}")
            self.metrics.errors += 1
            return False
    
    async def send_message(self, message: dict):
        """ """
        try:
            data = json.dumps(message).encode('utf-8')
            # 4   + 
            packet = struct.pack('!I', len(data)) + data
            
            self.socket.sendto(packet, (SERVER_HOST, SERVER_PORT))
            
            self.metrics.messages_sent += 1
            self.metrics.bytes_sent += len(packet)
        except Exception as e:
            self.metrics.errors += 1
            
    async def update_position(self):
        """ """
        #   
        self.position[0] += self.velocity[0]
        self.position[1] += self.velocity[1]
        
        #   
        for i in range(2):
            if self.position[i] < 0 or self.position[i] > WORLD_SIZE:
                self.velocity[i] *= -1
                self.position[i] = max(0, min(WORLD_SIZE, self.position[i]))
        
        #   
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
        """  """
        if not await self.connect():
            return
            
        try:
            while self.is_running:
                await self.update_position()
                await asyncio.sleep(UPDATE_INTERVAL)
        except Exception as e:
            print(f"{Fore.RED}Player {self.player_id} : {e}")
            self.metrics.errors += 1
        finally:
            await self.disconnect()
            
    async def disconnect(self):
        """ """
        if self.socket:
            try:
                message = {
                    "type": "disconnect",
                    "player_id": self.player_id,
                    "room_id": self.room_id,
                    "timestamp": time.time()
                }
                await self.send_message(message)
                self.socket.close()
            except:
                pass
    
    def stop(self):
        """ """
        self.is_running = False

class LoadTestManager:
    """  """
    
    def __init__(self):
        self.metrics = PerformanceMetrics()
        self.players: List[VirtualPlayer] = []
        self.is_running = True
        
    def print_header(self):
        """  """
        print(f"\n{Fore.CYAN}{'='*60}")
        print(f"{Fore.YELLOW}[!] RUDP Server Load Test")
        print(f"{Fore.CYAN}{'='*60}")
        print(f"[*] Test Environment: 1vCPU, 1GB RAM Simulation")
        print(f"[*] Rooms: {NUM_ROOMS}")
        print(f"[*] Players per room: {PLAYERS_PER_ROOM}")
        print(f"[*] Total players: {TOTAL_PLAYERS}")
        print(f"[*] Update interval: {UPDATE_INTERVAL*1000:.0f}ms")
        print(f"[*] Test duration: {TEST_DURATION}s")
        print(f"{Fore.CYAN}{'='*60}\n")
        
    async def create_players(self):
        """ """
        print(f"{Fore.GREEN}  ...")
        
        for room_id in range(NUM_ROOMS):
            for player_idx in range(PLAYERS_PER_ROOM):
                player_id = room_id * PLAYERS_PER_ROOM + player_idx
                player = VirtualPlayer(player_id, room_id, self.metrics)
                self.players.append(player)
                
            if (room_id + 1) % 10 == 0:
                print(f"   {room_id + 1}/{NUM_ROOMS}   ")
                
        print(f"{Fore.GREEN}  {len(self.players)}   \n")
        
    async def run_test(self):
        """ """
        self.print_header()
        await self.create_players()
        
        print(f"{Fore.YELLOW}  !")
        print(f"{Fore.CYAN}   ...\n")
        
        #   
        tasks = []
        batch_size = 50  #     
        
        for i in range(0, len(self.players), batch_size):
            batch = self.players[i:i+batch_size]
            for player in batch:
                task = asyncio.create_task(player.run())
                tasks.append(task)
            await asyncio.sleep(0.1)  #   
            
        #   
        monitor_task = asyncio.create_task(self.monitor_stats())
        
        #    
        await asyncio.sleep(TEST_DURATION)
        
        print(f"\n{Fore.YELLOW}   ...")
        self.is_running = False
        
        #   
        for player in self.players:
            player.stop()
            
        #  
        await asyncio.gather(*tasks, return_exceptions=True)
        monitor_task.cancel()
        
        #   
        self.print_results()
        
    async def monitor_stats(self):
        """  """
        while self.is_running:
            await asyncio.sleep(2)
            self.print_live_stats()
            
    def print_live_stats(self):
        """  """
        elapsed = time.time() - self.metrics.start_time
        
        #    
        sys.stdout.write('\033[5A')  # 5 
        sys.stdout.flush()
        
        print(f"    [{elapsed:.1f} ]")
        print(f"   : {self.metrics.messages_sent:,} msgs ({self.metrics.messages_per_second:.1f} msg/s)")
        print(f"   : {self.metrics.throughput_mbps:.2f} Mbps")
        print(f"    : {self.metrics.avg_latency:.1f}ms (: {self.metrics.min_latency:.1f}ms, : {self.metrics.max_latency:.1f}ms)")
        print(f"   : {self.metrics.errors}")
        
    def print_results(self):
        """  """
        elapsed = time.time() - self.metrics.start_time
        
        print(f"\n{Fore.CYAN}{'='*60}")
        print(f"{Fore.YELLOW}    ")
        print(f"{Fore.CYAN}{'='*60}")
        
        print(f"\n{Fore.GREEN}[ ]")
        print(f"    : {elapsed:.2f}")
        print(f"     : {NUM_ROOMS}")
        print(f"    : {TOTAL_PLAYERS}")
        print(f"    : {UPDATE_INTERVAL*1000:.0f}ms")
        
        print(f"\n{Fore.GREEN}[ ]")
        print(f"    : {self.metrics.messages_sent:,}")
        print(f"   /: {self.metrics.messages_per_second:.2f}")
        print(f"   : {self.metrics.throughput_mbps:.2f} Mbps")
        print(f"    : {self.metrics.bytes_sent:,}")
        
        print(f"\n{Fore.GREEN}[]")
        print(f"   : {self.metrics.avg_latency:.2f}ms")
        print(f"   : {self.metrics.min_latency:.2f}ms")
        print(f"   : {self.metrics.max_latency:.2f}ms")
        
        print(f"\n{Fore.GREEN}[]")
        print(f"    : {self.metrics.errors}")
        print(f"   : {(1 - self.metrics.errors/max(1, self.metrics.messages_sent)) * 100:.2f}%")
        
        #   
        self.print_system_resources()
        
        print(f"\n{Fore.CYAN}{'='*60}")
        
        #  
        self.evaluate_performance()
        
    def print_system_resources(self):
        """   """
        print(f"\n{Fore.GREEN}[ ]")
        
        # CPU 
        cpu_percent = psutil.cpu_percent(interval=1)
        print(f"   CPU : {cpu_percent:.1f}%")
        
        #  
        memory = psutil.virtual_memory()
        print(f"    : {memory.used / 1024**3:.2f}GB / {memory.total / 1024**3:.2f}GB ({memory.percent:.1f}%)")
        
        #   ( )
        try:
            for proc in psutil.process_iter(['pid', 'name', 'cpu_percent', 'memory_info']):
                if 'rudp' in proc.info['name'].lower():
                    mem_mb = proc.info['memory_info'].rss / 1024**2
                    print(f"   RUDP : CPU {proc.info['cpu_percent']:.1f}%,  {mem_mb:.1f}MB")
        except:
            pass
            
    def evaluate_performance(self):
        """ """
        print(f"\n{Fore.YELLOW}[ ]")
        
        msg_per_sec = self.metrics.messages_per_second
        avg_latency = self.metrics.avg_latency
        error_rate = self.metrics.errors / max(1, self.metrics.messages_sent)
        
        score = 0
        max_score = 0
        
        #    (: 10,000 msg/s)
        if msg_per_sec >= 10000:
            score += 30
            eval_msg = f"{Fore.GREEN}"
        elif msg_per_sec >= 5000:
            score += 20
            eval_msg = f"{Fore.YELLOW}"
        else:
            score += 10
            eval_msg = f"{Fore.RED} "
        max_score += 30
        print(f"    : {eval_msg} ({msg_per_sec:.0f} msg/s)")
        
        #   (: <50ms)
        if avg_latency <= 50:
            score += 30
            eval_lat = f"{Fore.GREEN}"
        elif avg_latency <= 100:
            score += 20
            eval_lat = f"{Fore.YELLOW}"
        else:
            score += 10
            eval_lat = f"{Fore.RED} "
        max_score += 30
        print(f"    : {eval_lat} ({avg_latency:.1f}ms)")
        
        #   (: <0.1%)
        if error_rate < 0.001:
            score += 40
            eval_err = f"{Fore.GREEN}"
        elif error_rate < 0.01:
            score += 25
            eval_err = f"{Fore.YELLOW}"
        else:
            score += 10
            eval_err = f"{Fore.RED} "
        max_score += 40
        print(f"   : {eval_err} ({error_rate*100:.3f}%)")
        
        # 
        total_score = (score / max_score) * 100
        if total_score >= 80:
            grade = f"{Fore.GREEN}A"
            comment = "   !"
        elif total_score >= 60:
            grade = f"{Fore.YELLOW}B"
            comment = "  "
        else:
            grade = f"{Fore.RED}C"
            comment = "  "
            
        print(f"\n  {Fore.CYAN} : {total_score:.0f}/100 (: {grade})")
        print(f"  {Fore.CYAN}: {comment}")

async def main():
    """ """
    manager = LoadTestManager()
    
    try:
        await manager.run_test()
    except KeyboardInterrupt:
        print(f"\n{Fore.YELLOW}   .")
    except Exception as e:
        print(f"\n{Fore.RED}   : {e}")

if __name__ == "__main__":
    # Windows     
    if sys.platform == 'win32':
        asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())
    
    asyncio.run(main())