const cors = require('cors');

/**
 * CORS 미들웨어 설정
 * - 모바일 앱을 위한 CORS 설정
 * - 허용된 출처, 메서드, 헤더 등을 설정
 */
const corsMiddleware = cors({
  // 허용된 출처 설정 (환경변수에서 가져오거나 모든 출처 허용)
  origin: process.env.ALLOWED_ORIGINS?.split(',') || '*',
  
  
  // 허용된 HTTP 메서드
  methods: ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'],
  
  // 허용된 헤더
  allowedHeaders: [
    'Content-Type',
    'Authorization',
    'X-Requested-With',
    'Accept',
    'Origin',
    'X-Platform',
    'X-OS-Version',
    'X-Device-Model',
    'X-App-Version'
  ],
  
  // 노출할 헤더
  exposedHeaders: ['X-API-Version'],
  
  // 인증 정보 포함 허용
  credentials: true,
  
  // preflight 요청 캐시 시간 (초)
  maxAge: 86400 // 24시간
});

module.exports = corsMiddleware; 