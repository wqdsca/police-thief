const path = require('path');
require('dotenv').config({ path: path.join(__dirname, '../../.env') });
const { Sequelize } = require('sequelize');

let sequelize = null;
let dbAvailable = false;

// MySQL 환경변수 확인
const mysqlConfig = {
  database: process.env.MYSQL_DB,
  username: process.env.MYSQL_USER,
  password: process.env.MYSQL_PASS,
  host: process.env.MYSQL_HOST,
  port: process.env.MYSQL_PORT,
};

// MySQL 설정이 완전한지 확인
const isMySQLConfigured = mysqlConfig.database && 
                         mysqlConfig.username && 
                         mysqlConfig.password && 
                         mysqlConfig.host && 
                         mysqlConfig.port;

if (isMySQLConfigured) {
  sequelize = new Sequelize(
    mysqlConfig.database,
    mysqlConfig.username,
    mysqlConfig.password,
    {
      host: mysqlConfig.host,
      port: mysqlConfig.port,
      dialect: 'mysql',
      dialectOptions: {
        charset: 'utf8mb4',
      },
      pool: {
        max: 5,
        min: 0,
        acquire: 30000,
        idle: 10000,
      },
      logging: false,
      define: {
        timestamps: true,
        underscored: false,
        paranoid: true,
      },
    }
  );
} else {
  console.log('⚠️ MySQL 설정이 불완전합니다. 메모리 모드로 실행됩니다.');
  console.log('MySQL 설정: MYSQL_DB, MYSQL_USER, MYSQL_PASS, MYSQL_HOST, MYSQL_PORT');
}

async function connectDB(retries = 3, delay = 2000) {
  if (!sequelize) {
    console.log('📊 데이터베이스: 메모리 모드 (MySQL 미설정)');
    return;
  }

  for (let i = 1; i <= retries; i++) {
    try {
      await sequelize.authenticate();
      console.log('✅ MySQL 연결 성공');
      dbAvailable = true;
      return;
    } catch (err) {
      console.warn(`❌ MySQL 연결 실패 (시도 ${i}/${retries}) - ${err.message}`);
      if (i === retries) {
        console.log('⚠️ MySQL 연결 실패. 메모리 모드로 실행됩니다.');
        console.log('💡 MySQL 설치: https://dev.mysql.com/downloads/mysql/');
        dbAvailable = false;
        return;
      }
      await new Promise(res => setTimeout(res, delay));
    }
  }
}

// 데이터베이스 상태 확인 함수
function isDatabaseAvailable() {
  return dbAvailable;
}

module.exports = { 
  sequelize, 
  connectDB, 
  isDatabaseAvailable,
  dbAvailable 
};
