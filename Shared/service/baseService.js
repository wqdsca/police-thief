const { Op } = require('sequelize');

/**
 * 📝 기본 서비스
 * - 공통 CRUD + Redis 연동
 */

class BaseService {
    constructor(repository, redisService, logger, options = {}) {
        this.repository = repository;
        this.redisService = redisService;
        this.logger = logger;
        this.options = options;
    }

    /**
     * 📝 생성
     * @param {Object} model - 모델 객체
     * @returns {Promise<boolean>}
     */
    async create(model) {
        try {
            const result = await this.repository.create(model);
            if (!result) throw new Error(`${this.options.name} DB 저장 실패`);
            await this.redisService.set(result);
            this.logger.info(`${this.options.name} 생성 성공`);
            return result.id;
        } catch (err) {
            this.logger.error(`${this.options.name} 생성 실패: ${err.message}`);
            return false;
        }
    }

    /**
     * 📝 수정
     * @param {Object} model - 모델 객체
     * @returns {Promise<boolean>}
     */
    async update(model) {
        try {
            const result = await this.repository.update(model, { where: { id: model.id } });
            if (!result) throw new Error(`${this.options.name} DB 수정 실패`);
            const updateModel = await this.repository.findById(model.id);
            if (this.redisService) await this.redisService.set(updateModel);
            this.logger.info(`${this.options.name} 수정 성공`);
            return true;
        } catch (err) {
            this.logger.error(`${this.options.name} 수정 실패: ${err.message}`);
            return false;
        }
    }

    /**
     * 🗑 단일 항목 삭제 (DB + Redis)
     * @param {number|string} id - 항목 ID
     * @returns {Promise<boolean>}
     */
    async delete(id) {
        try {
            const result = await this.repository.delete({ where: { id } });
            if (!result) throw new Error(`${this.options.name} DB 삭제 실패`);
            if (this.redisService) await this.redisService.delete(id);
            this.logger.info(`${this.options.name} 삭제 성공`);
            return true;
        } catch (err) {
            this.logger.error(`${this.options.name} 삭제 실패: ${err.message}`);
            return false;
        }
    }

    /**
     * 📋 목록 조회 (캐시 → DB fallback)
     * @param {string} key - Redis 키
     * @param {number|null} lastId - 마지막 ID (페이징 기준)
     * @param {number} limit - 최대 개수 (기본 20개)
     * @returns {Promise<Array|false>}
     */
    async getList(key, lastId = null, limit = 20) {
        try {
            let list = null;

            if (!lastId) {
                // 최신 목록 요청 시 Redis 사용
                list = await this.redisService.getList(key);
            }

            if (!list || list.length === 0) {
                list = await this.repository.findAll({
                    where: { id: { [Op.lt]: lastId || Number.MAX_SAFE_INTEGER } },
                    order: [['id', 'DESC']],
                    limit,
                });
            }

            if (!list || list.length === 0) {
                this.logger.warn(`${this.options.name} 목록 결과 없음`);
                return false;
            }

            if (!lastId && this.redisService) await this.redisService.setList(list); // 캐시 저장 (최신 목록만)
            return list;
        } catch (err) {
            this.logger.error(`${this.options.name} 목록 조회 실패: ${err.message}`);
            return false;
        }
    }

    /**
     * 🗑 리스트 캐시에서 항목 제거 (DB + Redis 리스트 삭제)
     * @param {string} key - Redis 리스트 키
     * @param {number|string} id - 삭제할 항목의 ID
     * @returns {Promise<boolean>}
     */
    async remove(key, id) {
        try {
            const result = await this.repository.delete(id);
            if (!result) throw new Error(`${this.options.name} DB 삭제 실패`);
            if (this.redisService) await this.redisService.remove(key, id); // Redis 리스트에서 제거
            this.logger.info(`${this.options.name} 리스트에서 제거 성공`);
            return true;
        } catch (err) {
            this.logger.error(`${this.options.name} 리스트 제거 실패: ${err.message}`);
            return false;
        }
    }

    async get(id, extraConditions = {}, attributes = null) {
        let result = null;
        try {
            result = await this.redisService.get(id);
            if (result) return result;
            result = await this.repository.findOne(
                { where: { userId: id, ...extraConditions } },
                null,
                attributes
            );
        } catch (err) {
            if (this.logger) this.logger.error(`${this.options?.name || '리소스'} 조회 실패: ${err.message}`);
            return null;
        }
        return result;
    }
}

module.exports = BaseService;
