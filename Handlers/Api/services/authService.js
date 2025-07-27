// Shared/service/Auth/AuthService.js
const axios = require('axios');
const jwt = require('jsonwebtoken');
const BaseService = require('../../../Shared/service/BaseService');
const BaseRepository = require('../../../Shared/repository/BaseRepository');
const redisUserService = require('../../../Shared/service/redis/redisUserService');
const tokenService = require('../../../Shared/service/Token/tokenService');

class AuthService extends BaseService {
  constructor(repository = BaseRepository, redisService = redisUserService, logger = console, options = {}) {
    super(repository, redisService, logger, options);
  }

  async login(loginType, token) {
    // 테스트만 하기위해서 코드
    if(loginType === 'test') {
        const id = 1; 
        const [accessToken, refreshToken] = await Promise.all([
            tokenService.generateAccessToken(id),
            tokenService.generateRefreshToken(id),
          ]);
          console.log("로그인 성공"
          );
        return {id: 1, nickname: 'test', isRegister: true, accessToken, refreshToken};
    }
    const userId = await this.getSocialUserId(loginType, token);
    const user = await this.repository.findOne({ where: { userId } });
    if (!user) return { isRegister: false };

    const isOnline = await this.redisService.get(user.id);
    if (isOnline) {
      this.logger.warn(`중복 로그인 차단: ${userId}`);
      const error = new Error('이미 로그인 된 유저입니다.');
      error.code = 'ALREADY_LOGGED_IN';
      throw error;
    }

    const { id, nickname } = user;
    return { id, nickname, isRegister: true };
  }

  async getSocialUserId(loginType, token) {
    switch (loginType) {
      case 'Kakao': {
        const { data } = await axios.get('https://kapi.kakao.com/v2/user/me', {
          headers: { Authorization: `Bearer ${token}` },
        });
        return `Kakao${data.id}`;
      }
      case 'Google': {
        const { data } = await axios.get('https://www.googleapis.com/oauth2/v3/userinfo', {
          headers: { Authorization: `Bearer ${token}` },
        });
        return `Google${data.sub}`;
      }
      case 'Apple': {
        const decoded = jwt.decode(token);
        if (!decoded?.sub) throw new Error('Apple 토큰이 유효하지 않습니다.');
        return `Apple${decoded.sub}`;
      }
      case 'test':
        return 'test';
      default:
        throw new Error('지원하지 않는 로그인 타입입니다.');
    }
  }
}

module.exports = new AuthService();
