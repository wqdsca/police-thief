const { redis } = require('../Shared/config/redis');

/**
 * Redis TTL 설정
 */
const TTL = {
    user: 60 * 30, // 30분
    room: 60 * 30, // 30분
    userList: 60 * 30, // 30분
    roomList: 60 * 30, // 30분
}

// const LIMITS = {
//     user: 100, // 100명
//     room: 100, // 100개
//     userList: 100, // 100명
//     roomList: 100, // 100개
// }

/**
 * Redis 키 생성 유틸리티
 */
const getKey = {
    //유저 관련
    user: (userId) => `user:${userId}`,

    roomInfo: (roomId) => `room:list:${roomId}`,
    roomUserList: (roomId) => `room:user:${roomId}`,
    roomListByTime: () => 'room:list:time',

}

/**
 * 재시도 유틸 (retryOperation)
 * - operation: 재시도할 비동기 함수 (Promise 반환)
 * - retries: 재시도 횟수 (최초 시도 + retries회 = 총 retries+1회 실행)
 * - delayMs: 첫 재시도 전 대기 시간 (밀리초)
 * - backoffFactor: 지수 백오프 계수
 */
async function retryOperation(operation, options = {}) {
    const { retries = 2, delayMs = 50, backoffFactor = 2 } = options;
    let attempt = 0;
    let lastError;
  
    while (attempt <= retries) {
      try {
        return await operation();
      } catch (err) {
        lastError = err;
        if (attempt === retries) break;
  
        console.warn(`Redis 작업 실패 (시도 ${attempt + 1}/${retries + 1}): ${err.message}`);
        const waitTime = delayMs * Math.pow(backoffFactor, attempt);
        await new Promise(res => setTimeout(res, waitTime));
        attempt += 1;
      }
    }
    throw lastError;
}

/**
 * Redis Pipeline 실행 유틸리티
 * - commands: [{method: 'set', args: ['key', 'value']}, ...] 형태의 명령어 배열
 */
async function pipeline(commands) {
    if (!Array.isArray(commands) || commands.length === 0) {
      return []; // 명령이 없으면 빈 배열 반환
    }
  
    try {
      const pipe = redis.pipeline();
  
      for (const cmd of commands) {
        const { method, args = [] } = cmd;
  
        if (typeof pipe[method] !== 'function') {
          console.error(`❌ 잘못된 Redis 명령어: ${method}`);
          throw new Error(`Invalid Redis command method: ${method}`);
        }
  
        pipe[method](...args); // args가 없으면 기본값 []로 처리
      }
  
      const results = await pipe.exec();

      // 실패한 명령이 있는지 확인 (에러가 존재하는 항목을 로깅)
      results.forEach(([err, res], idx) => {
        if (err) {
          console.error(`❌ Pipeline 실패 [${idx}]:`, err.message);
        }
      });
  
      return results;
    } catch (error) {
      console.error(`❌ Pipeline 전체 실패: ${error.message}`);
      throw error;
    }
  }
  

/**
 * Redis 연결 상태 확인
 */
async function healthCheck() {
    try {
        await redis.ping();
        return true;
    } catch (error) {
        console.error('Redis health check failed', error);
        return false;
    }
}

/**
 * 키 존재 여부 확인
 */
async function keyExists(key) {
    try {
        const exists = await redis.exists(key);
        return exists === 1;
    } catch (error) {
        console.error(`Key existence check failed for ${key}:`, error);
        throw error;
    }
}

/**
 * 키 삭제
 */
async function deleteKey(key) {
    try {
        const result = await redis.del(key);
        return result === 1;
    } catch (error) {
        console.error(`Key deletion failed for ${key}:`, error);
        throw error;
    }
}

/**
 * 키 만료 시간 설정
 */
async function setExpiry(key, seconds) {
    try {
        const result = await redis.expire(key, seconds);
        return result === 1;
    } catch (error) {
        console.error(`Expiry setting failed for ${key}:`, error);
        throw error;
    }
}

/**
 * 키 값 설정
 */
async function setValue(key, value) {
    try {
        const result = await redis.set(key, value);
        return result === 'OK';
    } catch (error) {
        console.error(`Value setting failed for ${key}:`, error);
        throw error;
    }
}

module.exports = {
    TTL,
    getKey,
    retryOperation,
    pipeline,
    healthCheck,
    keyExists,
    deleteKey,
    setExpiry,
    setValue
}