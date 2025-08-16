#!/usr/bin/env python3
"""
Rust 프로젝트 자동 문서화 스크립트
모든 public API에 대한 rustdoc 문서 생성
"""

import os
import re
from pathlib import Path
from typing import List, Dict, Optional

class DocumentationGenerator:
    def __init__(self, project_root: str):
        self.project_root = Path(project_root)
        self.templates = self.load_templates()
        self.stats = {
            'files_processed': 0,
            'items_documented': 0,
            'items_skipped': 0,
        }
    
    def load_templates(self) -> Dict[str, str]:
        """문서화 템플릿 정의"""
        return {
            'struct': '''/// {name} 구조체
/// 
/// {description}
/// 
/// # 필드
/// 
/// * `field1` - 필드 설명
/// 
/// # 예제
/// 
/// ```rust
/// use module::{name};
/// 
/// let instance = {name}::new();
/// ```''',
            
            'function': '''/// {name} 함수
/// 
/// {description}
/// 
/// # 인자
/// 
/// * `param1` - 매개변수 설명
/// 
/// # 반환값
/// 
/// 설명
/// 
/// # 오류
/// 
/// 가능한 오류 케이스
/// 
/// # 예제
/// 
/// ```rust
/// let result = {name}(param);
/// assert!(result.is_ok());
/// ```''',
            
            'trait': '''/// {name} 트레이트
/// 
/// {description}
/// 
/// # 구현 요구사항
/// 
/// 이 트레이트를 구현하려면...
/// 
/// # 예제
/// 
/// ```rust
/// struct MyType;
/// 
/// impl {name} for MyType {{
///     // 구현
/// }}
/// ```''',
            
            'enum': '''/// {name} 열거형
/// 
/// {description}
/// 
/// # Variants
/// 
/// * `Variant1` - 설명
/// 
/// # 예제
/// 
/// ```rust
/// use module::{name};
/// 
/// match value {{
///     {name}::Variant1 => {{}},
/// }}
/// ```''',
        }
    
    def find_undocumented_items(self, content: str) -> List[Dict]:
        """문서화되지 않은 public 항목 찾기"""
        items = []
        lines = content.split('\n')
        
        for i, line in enumerate(lines):
            # public 항목 찾기
            pub_match = re.match(r'^pub\s+(struct|fn|trait|enum|type|const|static)\s+(\w+)', line.strip())
            if pub_match:
                item_type = pub_match.group(1)
                item_name = pub_match.group(2)
                
                # 이전 줄에 문서 주석이 있는지 확인
                has_doc = False
                if i > 0:
                    prev_line = lines[i-1].strip()
                    if prev_line.startswith('///') or prev_line.startswith('//!'):
                        has_doc = True
                
                if not has_doc:
                    items.append({
                        'type': item_type,
                        'name': item_name,
                        'line': i + 1,
                        'code': line.strip()
                    })
        
        return items
    
    def generate_documentation(self, item: Dict) -> str:
        """항목에 대한 문서 생성"""
        item_type = item['type']
        item_name = item['name']
        
        # 타입별 설명 생성
        descriptions = {
            'struct': f"{item_name} 데이터 구조체",
            'fn': f"{item_name} 함수 구현",
            'trait': f"{item_name} 동작 정의",
            'enum': f"{item_name} 상태 열거형",
            'type': f"{item_name} 타입 별칭",
            'const': f"{item_name} 상수 값",
            'static': f"{item_name} 정적 변수"
        }
        
        description = descriptions.get(item_type, f"{item_name} 정의")
        
        # 템플릿 적용
        if item_type in self.templates:
            doc = self.templates[item_type].format(
                name=item_name,
                description=description
            )
        else:
            # 기본 문서화
            doc = f"/// {description}"
        
        return doc
    
    def process_file(self, filepath: Path) -> None:
        """파일 처리 및 문서화 추가"""
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()
            
            # 문서화되지 않은 항목 찾기
            undocumented = self.find_undocumented_items(content)
            
            if not undocumented:
                return
            
            lines = content.split('\n')
            offset = 0
            
            for item in undocumented:
                # 문서 생성
                doc = self.generate_documentation(item)
                doc_lines = doc.split('\n')
                
                # 문서 삽입
                insert_line = item['line'] - 1 + offset
                for doc_line in reversed(doc_lines):
                    lines.insert(insert_line, doc_line)
                    offset += 1
                
                self.stats['items_documented'] += 1
            
            # 파일 저장
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write('\n'.join(lines))
            
            print(f"✅ {filepath.name}: {len(undocumented)}개 항목 문서화")
            
        except Exception as e:
            print(f"❌ {filepath.name}: {e}")
    
    def generate_module_docs(self) -> None:
        """모듈 레벨 문서 생성"""
        readme_template = """# {module_name}

## 개요

{module_name} 모듈은 ...를 담당합니다.

## 주요 기능

- 기능 1
- 기능 2
- 기능 3

## 사용 예제

```rust
use {module_name}::{{...}};

// 예제 코드
```

## API 문서

자세한 API 문서는 `cargo doc --open`으로 확인하세요.

## 테스트

```bash
cargo test -p {module_name}
```

## 벤치마크

```bash
cargo bench -p {module_name}
```
"""
        
        # 각 워크스페이스 멤버에 대해 README 생성
        workspaces = ['shared', 'grpcserver', 'tcpserver', 'rudpserver', 'gamecenter']
        
        for workspace in workspaces:
            readme_path = self.project_root / workspace / 'README.md'
            if not readme_path.exists():
                content = readme_template.format(module_name=workspace)
                with open(readme_path, 'w', encoding='utf-8') as f:
                    f.write(content)
                print(f"📝 {workspace}/README.md 생성됨")
    
    def run(self) -> None:
        """전체 프로젝트 문서화"""
        print("📚 문서화 작업 시작...")
        print("="*50)
        
        # Rust 파일 찾기
        rust_files = []
        for root, dirs, files in os.walk(self.project_root):
            if 'target' in dirs:
                dirs.remove('target')
            
            for file in files:
                if file.endswith('.rs'):
                    rust_files.append(Path(root) / file)
        
        print(f"📁 {len(rust_files)}개 Rust 파일 발견\n")
        
        # 파일별 처리
        for filepath in rust_files:
            self.stats['files_processed'] += 1
            self.process_file(filepath)
        
        # 모듈 문서 생성
        self.generate_module_docs()
        
        # 결과 출력
        print("\n" + "="*50)
        print("📊 문서화 결과")
        print("="*50)
        print(f"처리된 파일: {self.stats['files_processed']}")
        print(f"문서화된 항목: {self.stats['items_documented']}")
        print(f"건너뛴 항목: {self.stats['items_skipped']}")

def main():
    import sys
    if len(sys.argv) < 2:
        print("Usage: python generate_docs.py <project_root>")
        sys.exit(1)
    
    project_root = sys.argv[1]
    generator = DocumentationGenerator(project_root)
    generator.run()

if __name__ == "__main__":
    main()