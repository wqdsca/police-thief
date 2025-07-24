const {hashHelper} = require('./redisBase');
const {getKey} = require('../../../Utils/redisUtils');

// TTL 포함한 캐시헬퍼 구성

const userSet = hashHelper(getKey.user, 60 * 30); //30분 

class RedisUserService {

    //온라인 유저 정보 설정
    async set(id, field, data, ttl = null) {
        await userSet.setField(id, field, data, ttl);
    }

    //온라인 유저 정보받기

    async get(id, field) {
        return await userSet.getField(id, field);
    }
    //온라인 유저 삭제

    async delete(id, field) {
        await userSet.deleteField(id, field);
    }

    async getList(id) {
        return await userSet.getAll(id);
    }

    async deleteList(id) {
        await userSet.delete(id);
    }

    async has(id, field) {
        return await userSet.exists(id, field);
    }
    async updateTTL(id, ttl) {
        await userSet.updateTTL(id, ttl);
    }
}

module.exports = new RedisUserService();