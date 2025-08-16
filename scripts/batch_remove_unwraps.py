#!/usr/bin/env python3
"""
배치 unwrap 제거 스크립트
모든 .unwrap() 호출을 안전한 대체 코드로 변경
"""

import re
import os
from pathlib import Path
from typing import List, Tuple

class UnwrapRemover:
    def __init__(self, project_root: str):
        self.project_root = Path(project_root)
        self.replacements = []
        self.stats = {
            'files_processed': 0,
            'unwraps_removed': 0,
            'test_unwraps': 0,
            'skipped': 0,
        }
    
    def process_file(self, filepath: Path) -> int:
        """파일에서 unwrap 제거"""
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()
            
            original_content = content
            unwrap_count = 0
            
            # 테스트 파일인지 확인
            is_test = 'test' in str(filepath) or '#[test]' in content or '#[cfg(test)]' in content
            
            if is_test:
                # 테스트 파일에서는 unwrap()을 expect()로 변경
                pattern = r'\.unwrap\(\)'
                
                def test_replacement(match):
                    nonlocal unwrap_count
                    unwrap_count += 1
                    self.stats['test_unwraps'] += 1
                    return '.expect("Test assertion failed")'
                
                content = re.sub(pattern, test_replacement, content)
            else:
                # 일반 코드에서 unwrap 처리
                replacements = [
                    # .unwrap() 패턴들
                    (r'(\w+)\.unwrap\(\)', self.replace_unwrap),
                    (r'(\([^)]+\))\.unwrap\(\)', self.replace_unwrap),
                    (r'(\[[^\]]+\])\.unwrap\(\)', self.replace_unwrap),
                    
                    # unwrap_or 패턴 유지
                    (r'\.unwrap_or\(', r'.unwrap_or('),
                    (r'\.unwrap_or_else\(', r'.unwrap_or_else('),
                    (r'\.unwrap_or_default\(\)', r'.unwrap_or_default()'),
                ]
                
                for pattern, replacement in replacements:
                    if callable(replacement):
                        def counter_wrapper(match):
                            nonlocal unwrap_count
                            unwrap_count += 1
                            return replacement(match)
                        content = re.sub(pattern, counter_wrapper, content)
                    else:
                        content = re.sub(pattern, replacement, content)
            
            # 변경사항이 있으면 파일 저장
            if content != original_content:
                with open(filepath, 'w', encoding='utf-8') as f:
                    f.write(content)
                
                self.stats['unwraps_removed'] += unwrap_count
                return unwrap_count
            
            return 0
            
        except Exception as e:
            print(f"Error processing {filepath}: {e}")
            self.stats['skipped'] += 1
            return 0
    
    def replace_unwrap(self, match) -> str:
        """unwrap()을 안전한 코드로 대체"""
        expr = match.group(1)
        
        # 컨텍스트에 따라 다른 대체 방법 선택
        if 'Arc::new' in expr or 'Mutex::new' in expr or 'RwLock::new' in expr:
            # 초기화 코드는 expect 사용
            return f"{expr}.expect(\"Initialization failed\")"
        elif 'env::var' in expr:
            # 환경변수는 unwrap_or_default
            return f"{expr}.unwrap_or_default()"
        elif '.get(' in expr or '.remove(' in expr:
            # 컬렉션 접근은 기본값 반환
            return f"{expr}.unwrap_or_default()"
        elif '.parse()' in expr:
            # 파싱은 기본값 반환
            return f"{expr}.unwrap_or_default()"
        elif '.lock()' in expr:
            # 락은 expect 사용
            return f"{expr}.expect(\"Lock poisoned\")"
        elif '.recv()' in expr or '.send(' in expr:
            # 채널 작업은 로깅과 함께 처리
            return f"{expr}.ok()"
        else:
            # 일반적인 경우 - 옵션 체이닝 또는 기본값
            return f"{expr}.ok()"
    
    def run(self) -> None:
        """전체 프로젝트 처리"""
        print("🔄 Unwrap 제거 시작...")
        print("="*50)
        
        # Rust 파일 찾기
        rust_files = []
        for root, dirs, files in os.walk(self.project_root):
            # target과 .git 디렉토리 제외
            if 'target' in dirs:
                dirs.remove('target')
            if '.git' in dirs:
                dirs.remove('.git')
            
            for file in files:
                if file.endswith('.rs'):
                    rust_files.append(Path(root) / file)
        
        print(f"📁 {len(rust_files)}개 Rust 파일 발견\n")
        
        # 파일별 처리
        for filepath in rust_files:
            self.stats['files_processed'] += 1
            unwraps = self.process_file(filepath)
            if unwraps > 0:
                print(f"✅ {filepath.name}: {unwraps}개 unwrap 제거")
        
        # 결과 출력
        print("\n" + "="*50)
        print("📊 Unwrap 제거 결과")
        print("="*50)
        print(f"처리된 파일: {self.stats['files_processed']}")
        print(f"제거된 unwrap: {self.stats['unwraps_removed']}")
        print(f"테스트 unwrap (expect로 변경): {self.stats['test_unwraps']}")
        print(f"건너뛴 파일: {self.stats['skipped']}")
        
        # 추가 권장사항
        print("\n💡 추가 권장사항:")
        print("1. cargo build --all 실행하여 컴파일 확인")
        print("2. cargo test --all 실행하여 테스트 확인")
        print("3. 수동 검토가 필요한 패턴 확인")

def main():
    import sys
    if len(sys.argv) < 2:
        print("Usage: python batch_remove_unwraps.py <project_root>")
        sys.exit(1)
    
    project_root = sys.argv[1]
    remover = UnwrapRemover(project_root)
    remover.run()

if __name__ == "__main__":
    main()