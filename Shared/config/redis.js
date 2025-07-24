/**
 * 실제 운영때 확인해야 하는 사항 
 * slack 알림 -> 관리자 페이지 개발 시 점검필요
 * Grafana 모니터링 확인 필요
 * 
 */
const Redis = require('ioredis');
const path = require('path');
// const axios = require('axios');
const client = require('prom-client');
require('dotenv').config({ path: path.resolve(__dirname, '../../.env') });

/**
 * 필수 Redis 환경변수 유효성 검사
 * - 환경 변수 누락 시 서버 실행 중단
 */
function validateRedisConfig() {
  const required = ['REDIS_HOST', 'REDIS_PORT'];
  const missing = required.filter(key => !process.env[key]);
  if (missing.length > 0) {
    throw new Error(`❌ 필수 Redis 환경변수가 누락되었습니다: ${missing.join(', ')}`);
  }
}
validateRedisConfig();

/**
 * Redis 연결 설정
 * - 보안, 안정성, 재시도 전략 포함
 */
const redisConfig = {
  host: process.env.REDIS_HOST,
  port: process.env.REDIS_PORT,
  password: process.env.REDIS_PASSWORD,
  db: process.env.REDIS_DB || 0,
  tls: process.env.REDIS_TLS === 'true' ? {} : undefined,

  enableOfflineQueue: true,
  enableReadyCheck: true,
  maxRetriesPerRequest: 3,
  connectTimeout: 10000,
  retryStrategy: (times) => Math.min(times * 50, 2000),
  reconnectOnError: (err) => err.message.includes('READONLY'),
  showFriendlyErrorStack: true
};


// Redis 클라이언트 생성
const redis = new Redis(redisConfig);

/**
 * 🔔 장애 발생 시 Slack 알림 전송
 * - 운영 Slack Webhook URL이 존재할 경우 실행됨
 */
// async function sendAlertToSlack(title, error) {
//   if (!process.env.SLACK_WEBHOOK_URL) return;

//   try {
//     await axios.post(process.env.SLACK_WEBHOOK_URL, {
//       text: `🚨 *${title}*\n> ${error.message}\n\`\`\`${error.stack || ''}\`\`\``
//     });
//     redisLogger.info('Slack 알림 전송 완료');
//   } catch (e) {
//     redisLogger.error('Slack 알림 전송 실패', { message: e.message });
//   }
// }

/**
 * 📊 Prometheus 메트릭: Redis 연결 상태 (1: 연결됨, 0: 끊김)
 */
const redisStatusGauge = new client.Gauge({
  name: 'redis_connected_status',
  help: 'Redis 연결 상태 (1: 정상, 0: 비정상)'
});

/**
 * 🔍 Redis 서버 상태 점검 (info 명령 기반)
 * - 버전, 모드 등 로그 출력
 */
async function checkRedisStatus() {
  try {
    const infoRaw = await redis.info();
    const info = Object.fromEntries(
      infoRaw.split('\n')
        .map(line => line.split(':'))
        .filter(pair => pair.length === 2)
    );
    console.log(`Redis 상태 확인 완료`, {
      version: info.redis_version,
      mode: info.redis_mode
    });
  } catch (err) {
    console.error('Redis 상태 점검 실패', {
      message: err.message,
      stack: err.stack
    });
  }
}

// Redis 연결 이벤트 핸들러
redis.on('connect', () => {
  console.log('✅ Redis 서버 연결 성공');
  redisStatusGauge.set(1);
  checkRedisStatus();
});

redis.on('ready', () => {
  console.log('🔄 Redis 클라이언트 준비 완료');
  checkRedisStatus();
});

redis.on('reconnecting', () => {
  console.warn('⚠️ Redis 재연결 시도 중...');
  redisStatusGauge.set(0);
});

redis.on('end', () => {
  console.warn('🛑 Redis 연결 종료됨');
  redisStatusGauge.set(0);
});

redis.on('error', (err) => {
  console.error('❌ Redis 에러 발생', {
    message: err.message,
    code: err.code,
    stack: err.stack
  });
  redisStatusGauge.set(0);

  // if (['ECONNREFUSED', 'ETIMEDOUT'].includes(err.code)) {
  //   sendAlertToSlack('Redis 연결 실패', err);
  // }
});

redis.on('close', () => {
  console.warn('Redis 연결 종료');
  redisStatusGauge.set(0);
});

/**
 * 🚪 Graceful Shutdown 처리
 * - SIGINT / SIGTERM 시 Redis 안전 종료
 */
async function shutdown() {
  console.log('🧹 Redis 종료 시도 중...');
  try {
    await redis.quit();
    console.log('✅ Redis 안전하게 종료됨');
  } catch (err) {
    console.error('Redis 종료 중 에러 발생', {
      message: err.message,
      stack: err.stack
    }); 
  } finally {
    process.exit(0);
  }
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

module.exports = {
  redis,
  redisStatusGauge
};
