# 📚 문서화 표준 가이드

## 🎯 목적
프로젝트 전체의 문서화를 한국어로 통일하고 일관성 있는 형식을 유지합니다.

## 📝 코드 주석 표준

### 모듈 문서화
```rust
//! # 모듈명
//!
//! ## 개요
//! 모듈의 주요 기능과 목적을 설명합니다.
//!
//! ## 주요 기능
//! - 기능 1: 설명
//! - 기능 2: 설명
//!
//! ## 사용 예시
//! ```rust
//! use module::function;
//! let result = function();
//! ```
```

### 함수 문서화
```rust
/// 함수의 간단한 설명
///
/// # 매개변수
/// - `param1`: 첫 번째 매개변수 설명
/// - `param2`: 두 번째 매개변수 설명
///
/// # 반환값
/// 반환값에 대한 설명
///
/// # 오류
/// 발생 가능한 오류 상황 설명
///
/// # 예시
/// ```rust
/// let result = function(arg1, arg2)?;
/// ```
///
/// # 주의사항
/// 특별히 주의해야 할 사항
pub fn function(param1: Type1, param2: Type2) -> Result<ReturnType> {
    // 구현
}
```

### 구조체 문서화
```rust
/// 구조체 설명
///
/// # 필드
/// - `field1`: 필드 설명
/// - `field2`: 필드 설명
///
/// # 예시
/// ```rust
/// let instance = StructName::new();
/// ```
#[derive(Debug, Clone)]
pub struct StructName {
    /// 필드 1 설명
    pub field1: Type1,
    
    /// 필드 2 설명
    pub field2: Type2,
}
```

## 🔄 문서화 변환 스크립트

```python
#!/usr/bin/env python3
# documentation_converter.py

import os
import re
from pathlib import Path

# 영어 -> 한국어 변환 사전
TRANSLATION_MAP = {
    # 일반 용어
    "TODO": "할일",
    "FIXME": "수정필요",
    "HACK": "임시방편",
    "NOTE": "참고",
    "WARNING": "경고",
    "IMPORTANT": "중요",
    
    # 문서화 키워드
    "Parameters": "매개변수",
    "Returns": "반환값",
    "Errors": "오류",
    "Example": "예시",
    "Examples": "예시",
    "Panics": "패닉",
    "Safety": "안전성",
    "Arguments": "인자",
    "Description": "설명",
    "Usage": "사용법",
    
    # 기술 용어
    "Connection": "연결",
    "Server": "서버",
    "Client": "클라이언트",
    "Message": "메시지",
    "Handler": "핸들러",
    "Manager": "관리자",
    "Service": "서비스",
    "Controller": "컨트롤러",
    "Repository": "저장소",
    "Database": "데이터베이스",
    "Cache": "캐시",
    "Queue": "큐",
    "Buffer": "버퍼",
    "Stream": "스트림",
    "Socket": "소켓",
    "Thread": "스레드",
    "Process": "프로세스",
    "Task": "작업",
    "Job": "작업",
    "Worker": "워커",
    
    # 동작 용어
    "Create": "생성",
    "Read": "읽기",
    "Update": "수정",
    "Delete": "삭제",
    "Get": "가져오기",
    "Set": "설정",
    "Add": "추가",
    "Remove": "제거",
    "Connect": "연결",
    "Disconnect": "연결 해제",
    "Send": "전송",
    "Receive": "수신",
    "Process": "처리",
    "Handle": "처리",
    "Validate": "검증",
    "Authenticate": "인증",
    "Authorize": "인가",
    
    # 상태 용어
    "Success": "성공",
    "Failure": "실패",
    "Error": "오류",
    "Warning": "경고",
    "Info": "정보",
    "Debug": "디버그",
    "Trace": "추적",
    "Fatal": "치명적",
    "Critical": "심각",
    "Major": "주요",
    "Minor": "경미",
    "Pending": "대기중",
    "Processing": "처리중",
    "Completed": "완료",
    "Failed": "실패",
    "Cancelled": "취소됨",
}

def convert_documentation(file_path: Path) -> bool:
    """
    Rust 파일의 문서화를 한국어로 변환합니다.
    """
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        original_content = content
        
        # 주석 패턴 찾기
        patterns = [
            (r'///\s*(.*)', '/// {}'),  # 문서 주석
            (r'//!\s*(.*)', '//! {}'),  # 모듈 주석
            (r'//\s*(.*)', '// {}'),    # 일반 주석
        ]
        
        for pattern, replacement in patterns:
            matches = re.finditer(pattern, content)
            for match in matches:
                comment_text = match.group(1)
                translated = translate_text(comment_text)
                if translated != comment_text:
                    content = content.replace(
                        match.group(0),
                        replacement.format(translated)
                    )
        
        # 변경사항이 있으면 파일 저장
        if content != original_content:
            with open(file_path, 'w', encoding='utf-8') as f:
                f.write(content)
            return True
        
        return False
        
    except Exception as e:
        print(f"오류 발생: {file_path} - {e}")
        return False

def translate_text(text: str) -> str:
    """
    텍스트를 한국어로 변환합니다.
    """
    result = text
    
    # 단어 단위로 변환
    for eng, kor in TRANSLATION_MAP.items():
        # 대소문자 구분 없이 변환
        result = re.sub(
            rf'\b{eng}\b',
            kor,
            result,
            flags=re.IGNORECASE
        )
    
    return result

def process_directory(directory: Path):
    """
    디렉토리 내 모든 Rust 파일을 처리합니다.
    """
    converted_count = 0
    total_count = 0
    
    for rust_file in directory.rglob("*.rs"):
        total_count += 1
        if convert_documentation(rust_file):
            converted_count += 1
            print(f"✅ 변환 완료: {rust_file}")
    
    print(f"\n📊 결과: {converted_count}/{total_count} 파일 변환됨")

if __name__ == "__main__":
    # 프로젝트 루트 디렉토리
    project_root = Path(".")
    process_directory(project_root)
```

## 📋 표준 용어집

### 게임 용어
| 영어 | 한국어 | 설명 |
|------|--------|------|
| Player | 플레이어 | 게임 참가자 |
| Character | 캐릭터 | 플레이어가 조작하는 캐릭터 |
| NPC | NPC | 논플레이어 캐릭터 |
| Monster | 몬스터 | 적대적 NPC |
| Item | 아이템 | 게임 내 물품 |
| Skill | 스킬 | 캐릭터 능력 |
| Quest | 퀘스트 | 임무 |
| Guild | 길드 | 플레이어 그룹 |
| Party | 파티 | 임시 플레이어 그룹 |
| PvP | PvP | 플레이어 대 플레이어 |
| PvE | PvE | 플레이어 대 환경 |

### 기술 용어
| 영어 | 한국어 | 설명 |
|------|--------|------|
| Thread-safe | 스레드 안전 | 동시성 안전성 |
| Lock-free | 락프리 | 락 없는 동시성 |
| Async | 비동기 | 비동기 처리 |
| Sync | 동기 | 동기 처리 |
| Concurrent | 동시성 | 동시 실행 |
| Parallel | 병렬 | 병렬 처리 |
| Atomic | 원자적 | 원자적 연산 |
| Mutex | 뮤텍스 | 상호 배제 |
| Channel | 채널 | 통신 채널 |
| Future | 퓨처 | 비동기 결과 |

### 상태 용어
| 영어 | 한국어 | 설명 |
|------|--------|------|
| Idle | 대기 | 유휴 상태 |
| Active | 활성 | 활동 상태 |
| Inactive | 비활성 | 비활동 상태 |
| Online | 온라인 | 접속 중 |
| Offline | 오프라인 | 접속 안함 |
| Connected | 연결됨 | 연결 상태 |
| Disconnected | 연결 끊김 | 연결 해제 상태 |
| Loading | 로딩중 | 불러오는 중 |
| Ready | 준비됨 | 준비 완료 |
| Busy | 바쁨 | 처리 중 |

## 🎨 문서화 템플릿

### README.md 템플릿
```markdown
# 프로젝트명

## 📖 개요
프로젝트에 대한 간단한 설명

## ✨ 주요 기능
- 기능 1
- 기능 2
- 기능 3

## 🚀 시작하기

### 필수 요구사항
- Rust 1.75+
- Redis 7.0+
- MariaDB 10.5+

### 설치
\```bash
git clone https://github.com/username/project.git
cd project
cargo build --release
\```

### 실행
\```bash
cargo run --bin server
\```

## 📚 문서
- [API 문서](docs/api.md)
- [아키텍처](docs/architecture.md)
- [기여 가이드](CONTRIBUTING.md)

## 📝 라이선스
MIT License
```

### API 문서 템플릿
```markdown
# API 문서

## 인증 API

### POST /api/auth/login
사용자 로그인

**요청**
\```json
{
  "username": "사용자명",
  "password": "비밀번호"
}
\```

**응답**
\```json
{
  "success": true,
  "token": "JWT 토큰",
  "user": {
    "id": 1,
    "username": "사용자명"
  }
}
\```

**오류 코드**
- 400: 잘못된 요청
- 401: 인증 실패
- 500: 서버 오류
```

## ✅ 체크리스트

### 문서화 완료 기준
- [ ] 모든 공개 API에 문서 주석 작성
- [ ] 모든 모듈에 개요 설명 추가
- [ ] 복잡한 로직에 설명 주석 추가
- [ ] README.md 작성 및 최신화
- [ ] API 문서 작성
- [ ] 아키텍처 문서 작성
- [ ] 예제 코드 제공
- [ ] 변경 로그 유지
- [ ] 라이선스 명시
- [ ] 기여 가이드 작성

## 🔄 자동화 도구

### pre-commit 훅 설정
```bash
#!/bin/sh
# .git/hooks/pre-commit

# 문서화 검사
cargo doc --no-deps --document-private-items
if [ $? -ne 0 ]; then
    echo "❌ 문서화 오류가 있습니다."
    exit 1
fi

# 한국어 문서화 확인
if grep -r "TODO\|FIXME" --include="*.rs" .; then
    echo "⚠️ TODO/FIXME를 한국어로 변경해주세요."
fi

echo "✅ 문서화 검사 통과"
```

### CI/CD 문서화 검증
```yaml
# .github/workflows/documentation.yml
name: Documentation Check

on: [push, pull_request]

jobs:
  doc-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: 문서 빌드
        run: cargo doc --all-features --no-deps
        
      - name: 문서 커버리지 확인
        run: |
          # 문서화되지 않은 항목 확인
          cargo doc --all-features --no-deps 2>&1 | grep "warning: missing documentation"
          
      - name: 문서 배포
        if: github.ref == 'refs/heads/main'
        run: |
          # GitHub Pages로 문서 배포
          cargo doc --all-features --no-deps
          echo '<meta http-equiv="refresh" content="0; url=project_name">' > target/doc/index.html
```

## 📊 문서화 메트릭

### 목표 지표
- 공개 API 문서화율: 100%
- 비공개 API 문서화율: 80%
- 코드 주석 비율: 20%
- 예제 코드 제공율: 90%
- 문서 최신화 주기: 1주

### 측정 도구
```bash
# 문서화 커버리지 측정
cargo doc-coverage

# 주석 비율 측정
tokei --type rust

# 문서 품질 검사
cargo deadlinks
```