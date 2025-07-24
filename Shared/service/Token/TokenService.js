const path = require('path');
require('dotenv').config({ path: path.join(__dirname, '../../../.env') });

const jwt = require('jsonwebtoken'); 

const ACCESS_TOKEN_SECRET = process.env.ACCESS_TOKEN_SECRET;
const REFRESH_TOKEN_SECRET = process.env.REFRESH_TOKEN_SECRET;
const accessTokenExpiresIn = '15m'; // 15분
const refreshTokenExpiresIn = '7d'; // 7일

class TokenService {
  static async generateAccessToken(id) {
    const payload = { id };
    const options = { expiresIn: accessTokenExpiresIn };
    const token = jwt.sign(payload, ACCESS_TOKEN_SECRET, options);
    return token;
  }
  
  static async generateRefreshToken(id) {
    const payload = { id };
    const options = { expiresIn: refreshTokenExpiresIn };
    return jwt.sign(payload, REFRESH_TOKEN_SECRET, options);
  }
  // api 용 토큰 검증 
  static async verifyAccessToken(req, res, next) {
    const AuthHeader = req.headers.authorization;
    if (!AuthHeader) {
      console.log('Authorization:', req.headers.authorization);

      return res.status(401).json({ message: '토큰이 없습니다.' });
    }

    const parts = AuthHeader.split(' ');
    if (parts.length !== 2 || parts[0] !== 'Bearer') {
      return res.status(401).json({ message: '토큰 형식이 올바르지 않습니다.' });
    }

    const token = parts[1];
    try {
      const decoded = jwt.verify(token, ACCESS_TOKEN_SECRET);
      req.userId = decoded.id; // ⬅️ userId를 다음 미들웨어로 넘겨줌
      next();
    } catch (error) {
      return res.status(401).json({ message: '토큰이 만료되었거나 유효하지 않습니다.' });
    }
  }

  static async verifyRefreshToken(req, res, next) {
    const userId = req.userId;

    try {
      const result = await authService.getRefreshTokenByUserId(userId);
      const refreshToken = result?.refreshToken;

      if (!refreshToken) {
 
        return res.status(401).json({ message: '리프레시 토큰이 없습니다.' });
      }

      jwt.verify(refreshToken, REFRESH_TOKEN_SECRET);
      next();
    } catch (error) {
      return res.status(401).json({ message: '리프레시 토큰이 만료되었거나 유효하지 않습니다.' });
    }
  }

  // TokenService.js 내부에 추가
static verifySocketToken(socket, next) {
  try {
  
    const rawToken = socket.handshake.auth.Authorization;
    const token = rawToken?.replace('Bearer ', '');
    if (!token) {
      return next(new Error('토큰이 없습니다.'));
    }
    const decoded = jwt.verify(token, ACCESS_TOKEN_SECRET);
    socket.userId = decoded.id; 
    next();
  } catch (error) {
    return next(new Error('유효하지 않은 토큰입니다.'));
  }
}
}

module.exports = TokenService;
