/**
 * redisBase.js
 * 공통 Redis 헬퍼 묶음
 */

const { redis } = require('../../config/redis');
const {
  TTL,
  getKey,          // 기존 key generator 모듈 (필요한 keyFn 새로 정의해서 넘김)
  retryOperation,
  pipeline,
} = require('../../../Utils/redisUtils');

/* ─────────────────────────────────────────────────────────────
 * 공통 옵션
 * ────────────────────────────────────────────────────────────*/
const RETRY_OPT = { retries: 2, delayMs: 50, backoffFactor: 2 };

/* ─────────────────────────────────────────────────────────────
 * cacheHelper (LIST + 단건 JSON)
 *  - 리스트는 최신순(HEAD=lpush)
 *  - limit 초과 시 rpop + 단건키 삭제
 *  - listId 없는 경우: listKeyFn() 사용
 * ────────────────────────────────────────────────────────────*/
const cacheHelper = (itemKeyFn, listKeyFn, ttl = null, limit = 1000) => {
  if (typeof itemKeyFn !== 'function' || typeof listKeyFn !== 'function') {
    throw new Error('cacheHelper: itemKeyFn / listKeyFn 함수 필요');
  }

  return {
    /**
     * 단건 추가 + 목록 최신화
     */
    async add(id, data, listId = null) {
      return retryOperation(async () => {
        const itemKey = itemKeyFn(id);
        const listKey = listId != null ? listKeyFn(listId) : listKeyFn();
        const json = JSON.stringify(data);

        // 파이프라인: SET / LREM / LPUSH
        const cmds = [];
        if (ttl) {
          cmds.push({ method: 'set', args: [itemKey, json, 'EX', ttl] });
        } else {
          cmds.push({ method: 'set', args: [itemKey, json] });
        }
        cmds.push(
          { method: 'lrem', args: [listKey, 0, id] },
          { method: 'lpush', args: [listKey, id] },
          { method: 'ltrim', args: [listKey, 0, limit - 1] },
        );
        await pipeline(cmds);

        // 리스트 TTL 갱신
        if (ttl) await redis.expire(listKey, ttl);

        // 길이 초과 요소 제거 (rpop)
        const len = await redis.llen(listKey);
        if (len > limit) {
          const overId = await redis.rpop(listKey);
            // 오래된 아이템 단건 키 삭제
          if (overId) await redis.del(itemKeyFn(overId));
        }
        return true;
      }, RETRY_OPT).catch(err => {
        console.error(`cacheHelper.add 실패 (id=${id}): ${err.message}`);
        return false;
      });
    },

    /**
     * 리스트 세트 (기존 목록 초기화 후 재적재)
     * items: [{ id, data }, ...]
     */
    async setList(items, listId = null) {
      if (!Array.isArray(items) || items.length === 0) return false;
      return retryOperation(async () => {
        const listKey = listId != null ? listKeyFn(listId) : listKeyFn();
        await redis.del(listKey);

        const slice = items.slice(0, limit);
        const cmds = [];
        for (const { id, data } of slice) {
          const k = itemKeyFn(id);
            if (ttl) cmds.push({ method: 'set', args: [k, JSON.stringify(data), 'EX', ttl] });
            else cmds.push({ method: 'set', args: [k, JSON.stringify(data)] });
          cmds.push({ method: 'rpush', args: [listKey, id] }); // rpush → 입력 순서 유지
        }
        cmds.push({ method: 'ltrim', args: [listKey, 0, limit - 1] });
        await pipeline(cmds);
        if (ttl) await redis.expire(listKey, ttl);
        return true;
      }, RETRY_OPT).catch(err => {
        console.error(`cacheHelper.setList 실패: ${err.message}`);
        return false;
      });
    },

    /**
     * 목록 조회 (listId 선택)
     */
    async getList(listId = null) {
      return retryOperation(async () => {
        const listKey = listId != null ? listKeyFn(listId) : listKeyFn();
        const ids = await redis.lrange(listKey, 0, limit - 1);
        if (ids.length === 0) return [];

        const pipe = redis.pipeline();
        ids.forEach(id => pipe.get(itemKeyFn(id)));
        const results = await pipe.exec();

        const valid = [];
        const invalidIds = [];
        for (let i = 0; i < results.length; i++) {
          const [err, val] = results[i];
          if (err || !val) invalidIds.push(ids[i]);
          else valid.push(JSON.parse(val));
        }
        if (invalidIds.length) {
          const pipe2 = redis.pipeline();
          invalidIds.forEach(bad => pipe2.lrem(listKey, 0, bad));
          await pipe2.exec();
        }
        return valid;
      }, RETRY_OPT).catch(err => {
        console.error(`cacheHelper.getList 실패: ${err.message}`);
        return null;
      });
    },

    /**
     * 단건 조회
     */
    async get(id) {
      return retryOperation(async () => {
        const v = await redis.get(itemKeyFn(id));
        return v ? JSON.parse(v) : null;
      }, RETRY_OPT).catch(err => {
        console.error(`cacheHelper.get 실패 (id=${id}): ${err.message}`);
        return null;
      });
    },

    /**
     * 단건 삭제 (특정 listId 명시 가능)
     */
    async delete(id, listId = null) {
      return retryOperation(async () => {
        await redis.del(itemKeyFn(id));
        const listKey = listId != null ? listKeyFn(listId) : listKeyFn();
        await redis.lrem(listKey, 0, id);
        return true;
      }, RETRY_OPT).catch(err => {
        console.error(`cacheHelper.delete 실패 (id=${id}): ${err.message}`);
        return false;
      });
    },
  };
};

/* ─────────────────────────────────────────────────────────────
 * setHelper (Set 구조)
 * ────────────────────────────────────────────────────────────*/
const setHelper = (setKeyFn, ttl = null) => {
  if (typeof setKeyFn !== 'function') throw new Error('setHelper: setKeyFn 필요');
  return {
    async add(id, value) {
      return retryOperation(async () => {
        const key = setKeyFn(id);
        await redis.sadd(key, value);
        if (ttl) await redis.expire(key, ttl);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`setHelper.add 실패 (key=${setKeyFn(id)}): ${e.message}`);
        return false;
      });
    },
    async remove(id, value) {
      return retryOperation(async () => {
        const key = setKeyFn(id);
        await redis.srem(key, value);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`setHelper.remove 실패 (key=${setKeyFn(id)}): ${e.message}`);
        return false;
      });
    },
    async has(id, value) {
      return retryOperation(async () => {
        const key = setKeyFn(id);
        return (await redis.sismember(key, value)) === 1;
      }, RETRY_OPT).catch(e => {
        console.error(`setHelper.has 실패 (key=${setKeyFn(id)}): ${e.message}`);
        return false;
      });
    },
    async getAll(id) {
      return retryOperation(async () => {
        const key = setKeyFn(id);
        return await redis.smembers(key);
      }, RETRY_OPT).catch(e => {
        console.error(`setHelper.getAll 실패 (key=${setKeyFn(id)}): ${e.message}`);
        return null;
      });
    },
    async count(id) {
      return retryOperation(async () => {
        const key = setKeyFn(id);
        return await redis.scard(key);
      }, RETRY_OPT).catch(e => {
        console.error(`setHelper.count 실패 (key=${setKeyFn(id)}): ${e.message}`);
        return null;
      });
    },
    async updateTTL(id, newTTL) {
      if (!newTTL) return true;
      return retryOperation(async () => {
        const key = setKeyFn(id);
        await redis.expire(key, newTTL);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`setHelper.updateTTL 실패 (key=${setKeyFn(id)}): ${e.message}`);
        return false;
      });
    },
    async delete(id) {
      return retryOperation(async () => {
        await redis.del(setKeyFn(id));
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`setHelper.delete 실패 (key=${setKeyFn(id)}): ${e.message}`);
        return false;
      });
    },
  };
};

/* ─────────────────────────────────────────────────────────────
 * zsetHelper (Sorted Set)
 * ────────────────────────────────────────────────────────────*/
const zsetHelper = (zsetKeyFn, ttl = null) => {
  if (typeof zsetKeyFn !== 'function') throw new Error('zsetHelper: zsetKeyFn 필요');
  return {
    async add(id, score, member) {
      return retryOperation(async () => {
        const key = zsetKeyFn(id);
        await redis.zadd(key, score, member);
        if (ttl) await redis.expire(key, ttl);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.add 실패: ${e.message}`);
        return false;
      });
    },
    async range(id, start, stop, withScores = false) {
      return retryOperation(async () => {
        const key = zsetKeyFn(id);
        if (!withScores) return await redis.zrange(key, start, stop);
        const raw = await redis.zrange(key, start, stop, 'WITHSCORES');
        const out = [];
        for (let i = 0; i < raw.length; i += 2) {
          out.push({ member: raw[i], score: Number(raw[i + 1]) });
        }
        return out;
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.range 실패: ${e.message}`);
        return null;
      });
    },
    async revRange( start, stop, withScores = false) {
      const key = zsetKeyFn();
      return retryOperation(async () => {
        if (!withScores) return await redis.zrevrange(key, start, stop);
        const raw = await redis.zrevrange(key, start, stop, 'WITHSCORES');
        const out = [];
        for (let i = 0; i < raw.length; i += 2) {
          out.push({ member: raw[i], score: Number(raw[i + 1]) });
        }
        return out;
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.revRange 실패: ${e.message}`);
        return null;
      });
    },
    async score(member) {
      return retryOperation(async () => {
        const key = zsetKeyFn();
        const s = await redis.zscore(key, member);
        return s;
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.score 실패: ${e.message}`);
        return null;
      });
    },
    async revRangeByScore(max, min, limit = 20, withScores = false) {
      const key = zsetKeyFn();
    
      console.log('[revRangeByScore] key:', key);
      console.log('[revRangeByScore] max:', max, typeof max);
      console.log('[revRangeByScore] min:', min, typeof min);
      console.log('[revRangeByScore] limit:', limit);
      console.log('[revRangeByScore] withScores:', withScores);
    
      return retryOperation(async () => {
        const maxStr = `${max}`;
        const minStr = `${min}`;
    
        console.log('[revRangeByScore] converted maxStr:', maxStr);
        console.log('[revRangeByScore] converted minStr:', minStr);
    
        try {
          if (!withScores) {
            const result = await redis.zrevrangebyscore(key, maxStr, minStr, 'LIMIT', 0, limit);
            console.log('[revRangeByScore] result:', result);
            return result;
          }
    
          const raw = await redis.zrevrangebyscore(key, maxStr, minStr, 'WITHSCORES', 'LIMIT', 0, limit);
          console.log('[revRangeByScore] raw:', raw);
    
          const out = [];
          for (let i = 0; i < raw.length; i += 2) {
            out.push({ member: raw[i], score: Number(raw[i + 1]) });
          }
          return out;
        } catch (e) {
          console.error('[revRangeByScore] Redis 오류:', e.message);
          throw e;
        }
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.revRangeByScore 실패: ${e.message}`);
        return null;
      });
    }
    
    
    ,
    
    async rank(id, member) {
      return retryOperation(async () => {
        const key = zsetKeyFn(id);
        return await redis.zrank(key, member);
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.rank 실패: ${e.message}`);
        return null;
      });
    },
    async remove(id, member) {
      return retryOperation(async () => {
        const key = zsetKeyFn(id);
        await redis.zrem(key, member);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.remove 실패: ${e.message}`);
        return false;
      });
    },
    async count(id) {
      return retryOperation(async () => {
        const key = zsetKeyFn(id);
        return await redis.zcard(key);
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.count 실패: ${e.message}`);
        return null;
      });
    },
    async delete(id) {
      return retryOperation(async () => {
        await redis.del(zsetKeyFn(id));
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`zsetHelper.delete 실패: ${e.message}`);
        return false;
      });
    },
  };
};

/* ─────────────────────────────────────────────────────────────
 * hashHelper (Hash)
 * ────────────────────────────────────────────────────────────*/
const hashHelper = (hashKeyFn, ttl = null) => {
  if (typeof hashKeyFn !== 'function') throw new Error('hashHelper: hashKeyFn 필요');

  const toStr = v => (typeof v === 'object' ? JSON.stringify(v) : String(v));
  const safeParse = v => {
    if (v === null || v === undefined) return null;
    if (v === '' || v === 'undefined') return null;
    try {
      return JSON.parse(v);
    } catch {
      return v;
    }
  };

  return {
    async setField(id, field, value, ttl = null) {
      return retryOperation(async () => {
        const key = hashKeyFn(id);
        await redis.hset(key, field, JSON.stringify(value));
        if (ttl) await redis.expire(key, ttl);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.setField 실패: ${e.message}`);
        return false;
      });
    },
    async getField(id, field) {
      return retryOperation(async () => {
        const v = await redis.hget(hashKeyFn(id), field);
        return v === null ? null : safeParse(v);
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.getField 실패: ${e.message}`);
        return null;
      });
    },
    async getAll(id) {
      return retryOperation(async () => {
        const key = hashKeyFn(id);
        const raw = await redis.hgetall(key);
        if (!raw || Object.keys(raw).length === 0) return null;
        const cleaned = {};
        // 유령 필드 제거 파이프라인
        const pipe = redis.pipeline();
        for (const [f, v] of Object.entries(raw)) {
          const parsed = safeParse(v);
          if (parsed === null) {
            pipe.hdel(key, f);
          } else {
            cleaned[f] = parsed;
          }
        }
        if (pipe.commands) await pipe.exec();
        return cleaned;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.getAll 실패: ${e.message}`);
        return null;
      });
    },
    async setList(id, dataMap) {
      return retryOperation(async () => {
        const key = hashKeyFn(id);
        const serialized = {};
        for (const [f, v] of Object.entries(dataMap)) {
          serialized[f] = toStr(v);
        }
        await redis.hset(key, serialized);
        if (ttl) await redis.expire(key, ttl);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.setList 실패: ${e.message}`);
        return false;
      });
    },
    async deleteField(id, field) {
      return retryOperation(async () => {
        await redis.hdel(hashKeyFn(id), field);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.deleteField 실패: ${e.message}`);
        return false;
      });
    },
    async delete(id) {
      return retryOperation(async () => {
        await redis.del(hashKeyFn(id));
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.delete 실패: ${e.message}`);
        return false;
      });
    },
    async exists(id, field) {
      return retryOperation(async () => {
        return (await redis.hexists(hashKeyFn(id), field)) === 1;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.exists 실패: ${e.message}`);
        return false;
      });
    },
    async updateTTL(id, ttl) {
      if (!ttl) return true;
      return retryOperation(async () => {
        const key = hashKeyFn(id);
        await redis.expire(key, ttl);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.updateTTL 실패: ${e.message}`);
        return false;
      });
    },
    async hincrby(id, field, value) {
      return retryOperation(async () => {
        const key = hashKeyFn(id);
        await redis.hincrby(key, field, value);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`hashHelper.hincrby 실패: ${e.message}`);
        return false;
      });
    },
    // 여러가지 필드 가져올때 
    async mget(id, ...fields) {
      return retryOperation(async () => {
        const key = hashKeyFn(id);
        const raw = await redis.hmget(key, ...fields);
        const out = {};
        for(let i = 0; i < fields.length; i++) {
          out[fields[i]] = safeParse(raw[i]);
        }
        return out;

      })
    }
  };
};

/* ─────────────────────────────────────────────────────────────
 * geoHelper (Geo) - GEOSEARCH 사용
 * ────────────────────────────────────────────────────────────*/
const geoHelper = (geoKeyFn, ttl = null) => {
  if (typeof geoKeyFn !== 'function') throw new Error('geoHelper: geoKeyFn 필요');

  return {
    async add(id, longitude, latitude, member) {
      return retryOperation(async () => {
        const key = geoKeyFn(id);
        await redis.geoadd(key, longitude, latitude, member);
        if (ttl) await redis.expire(key, ttl);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`geoHelper.add 실패: ${e.message}`);
        return false;
      });
    },

    /**
     * 반경 검색 (GEOSEARCH)
     * returns: [{ member, distance?, coordinates? }, ...]
     */
    async radius(id, longitude, latitude, radius, unit = 'km', withCoord = false, withDist = false, count = null) {
      return retryOperation(async () => {
        const key = geoKeyFn(id);
        const args = ['GEOSEARCH', key, 'FROMLONLAT', longitude, latitude, 'BYRADIUS', radius, unit];
        if (withDist) args.push('WITHDIST');
        if (withCoord) args.push('WITHCOORD');
        if (count) args.push('COUNT', count);
        // raw reply
        const raw = await redis.sendCommand(args);

        // 파싱
        // WITHDIST/COORD 조합에 따라 구조 달라짐
        // Node-redis에서 반환 형식이 배열(중첩)일 수 있으니 정규화 진행
        const normalize = [];
        raw.forEach(entry => {
            if (typeof entry === 'string') {
              // no WITHDIST/COORD
              normalize.push({ member: entry });
            } else if (Array.isArray(entry)) {
              // 다양한 조합
              // 패턴: [member, dist?, [lon,lat]?]
              let member = entry[0];
              let distance = null;
              let coordinates = null;
              if (withDist && withCoord) {
                distance = parseFloat(entry[1]);
                coordinates = [parseFloat(entry[2][0]), parseFloat(entry[2][1])];
              } else if (withDist) {
                distance = parseFloat(entry[1]);
              } else if (withCoord) {
                coordinates = [parseFloat(entry[1][0]), parseFloat(entry[1][1])];
              }
              normalize.push({ member, distance, coordinates });
            }
        });
        return normalize;
      }, RETRY_OPT).catch(e => {
        console.error(`geoHelper.radius 실패: ${e.message}`);
        return null;
      });
    },

    async pos(id, member) {
      return retryOperation(async () => {
        const key = geoKeyFn(id);
        return await redis.geopos(key, member); // [[lon, lat]] or [null]
      }, RETRY_OPT).catch(e => {
        console.error(`geoHelper.pos 실패: ${e.message}`);
        return null;
      });
    },

    async dist(id, member1, member2, unit = 'km') {
      return retryOperation(async () => {
        const key = geoKeyFn(id);
        return await redis.geodist(key, member1, member2, unit);
      }, RETRY_OPT).catch(e => {
        console.error(`geoHelper.dist 실패: ${e.message}`);
        return null;
      });
    },

    async remove(id, member) {
      return retryOperation(async () => {
        const key = geoKeyFn(id);
        await redis.zrem(key, member);
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`geoHelper.remove 실패: ${e.message}`);
        return false;
      });
    },

    async delete(id) {
      return retryOperation(async () => {
        await redis.del(geoKeyFn(id));
        return true;
      }, RETRY_OPT).catch(e => {
        console.error(`geoHelper.delete 실패: ${e.message}`);
        return false;
      });
    },
  };
};

/* ─────────────────────────────────────────────────────────────
 * EXPORTS
 * ────────────────────────────────────────────────────────────*/
module.exports = {
  redis,
  TTL,
  getKey,
  retryOperation,
  pipeline,
  cacheHelper,
  setHelper,
  zsetHelper,
  hashHelper,
  geoHelper,
};
