/**
 * 💾 BaseRepository
 * - Sequelize 기반 공통 DB Repository
 * - 단건/다건 생성, 조회, 수정, 삭제
 * - 트랜잭션 처리, 페이징, 카운트, 증가/감소 등 기능 포함
 */

class BaseRepository {
    constructor(model) {
      this.model = model;
    }
  
    /**
     * ✅ 단일 데이터 생성
     * @param {Object} data - 저장할 데이터 객체
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise<Object>} 생성된 레코드
     */
    async create(data, transaction = null) {
      return await this.model.create(data, { transaction });
    }
  
    /**
     * ✅ 여러 데이터 한 번에 생성 (bulk insert)
     * @param {Object[]} dataList - 데이터 배열
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @param {Object} options - validate, ignoreDuplicates 등 옵션
     * @returns {Promise<Object[]>} 생성된 레코드 배열
     */
    async bulkCreate(dataList, transaction = null, options = {}) {
      return await this.model.bulkCreate(dataList, {
        transaction,
        ...options,
      });
    }
  
    /**
     * ✅ bulk insert + 트랜잭션 자동 처리
     * - 모두 성공 시 commit
     * - 하나라도 실패 시 rollback
     * @param {Object[]} dataList - 데이터 배열
     * @param {Sequelize} sequelize - Sequelize 인스턴스 (트랜잭션 생성용)
     * @param {Object} options - bulkCreate 옵션
     * @returns {Promise<Object[]>} 생성된 레코드 배열
     */
    async bulkCreateWithTransaction(dataList, sequelize, options = {}) {
      const t = await sequelize.transaction();
      try {
        const result = await this.model.bulkCreate(dataList, {
          transaction: t,
          ...options,
        });
        await t.commit();
        return result;
      } catch (err) {
        await t.rollback();
        throw err;
      }
    }
  
    /**
     * ✅ PK로 단건 조회
     * @param {any} pk - 기본키 값
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise<Object|null>} 결과 1건 또는 null
     */
    async findByPk(pk, transaction = null) {
      return await this.model.findByPk(pk, { transaction });
    }
  
    /**
     * ✅ 조건 기반 다건 조회
     * @param {Object} options - Sequelize 옵션 (where, order 등)
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise<Object[]>} 결과 배열
     */
    async findAll(options = {}, transaction = null, limit = null) {
      return await this.model.findAll({ ...options, transaction, limit });
    }
  
    /**
     * ✅ 조건 기반 단건 조회
     * @param {Object} options - Sequelize 옵션 (where 등)
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise<Object|null>} 결과 1건 또는 null
     */
    async findOne(options = {}, transaction = null, attributes = null) {
      const query = { ...options };
      if(transaction) query.transaction = transaction;
      if(attributes) query.attributes = attributes;
      return await this.model.findOne(query);
    }

        /**
     * userId로 단건 조회 (트랜잭션 optional)
     * - userId 필드를 사용하는 모델에서만 사용 가능
     * @param {string} userId 
     * @param {Object|null} transaction 
     * @returns {Promise<Object|null>}
     */
    async findByUserId(userId, transaction = null) {
        return await this.model.findOne({ where: { userId }, transaction });
    }
    
    /**
     * ✅ 조건 기반 수정
     * @param {Object} data - 수정할 필드들
     * @param {Object} where - 수정 조건
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise<[number, Object[]]>} 영향받은 행 수 및 결과
     */
    async update(data, where, transaction = null) {
      return await this.model.update(data, { where, transaction });
    }
  
    /**
     * ✅ 조건 기반 삭제
     * @param {Object} where - 삭제 조건
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise<number>} 삭제된 행 수
     */
    async delete(where, transaction = null) {
      return await this.model.destroy({ where, transaction });
    }
  
    /**
     * ✅ 조건 기반 개수 조회
     * @param {Object} where - 조건 객체
     * @returns {Promise<number>} 총 개수
     */
    async count(where = {}) {
      return await this.model.count({ where });
    }
  
    // /**
    //  * ✅ 페이징 처리
    //  * @param {number} page - 현재 페이지 (1부터 시작)
    //  * @param {number} pageSize - 페이지당 개수
    //  * @param {Object} options - where, order 등 Sequelize 옵션
    //  * @returns {Promise<{ count: number, rows: Object[] }>} 전체 개수 + 결과 목록
    //  */
    // async paginate(page = 1, pageSize = 10, options = {}) {
    //   const offset = (page - 1) * pageSize;
    //   return await this.model.findAndCountAll({
    //     offset,
    //     limit: pageSize,
    //     ...options,
    //   });
    // }
  
    /**
     * ✅ 숫자 필드 증가
     * @param {string|string[]} field - 증가할 필드명
     * @param {number} by - 증가량 (기본 1)
     * @param {Object} where - 조건
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise} Sequelize increment 결과
     */
    async increment(field, by = 1, where, transaction = null) {
      return await this.model.increment(field, { by, where, transaction });
    }
  
    /**
     * ✅ 숫자 필드 감소
     * @param {string|string[]} field - 감소할 필드명
     * @param {number} by - 감소량 (기본 1)
     * @param {Object} where - 조건
     * @param {Object|null} transaction - 트랜잭션 객체 (optional)
     * @returns {Promise} Sequelize decrement 결과
     */
    async decrement(field, by = 1, where, transaction = null) {
      return await this.model.decrement(field, { by, where, transaction });
    }
  }
  
  module.exports = new BaseRepository();
  