#!/usr/bin/env python3
"""
unwrap() 자동 제거 스크립트
프로젝트 전체의 unwrap()를 안전한 에러 처리로 변환
"""

import os
import re
import sys
from pathlib import Path
from typing import List, Tuple

class UnwrapRemover:
    def __init__(self, project_root: str):
        self.project_root = Path(project_root)
        self.stats = {
            'files_processed': 0,
            'unwraps_found': 0,
            'unwraps_fixed': 0,
            'errors': []
        }
    
    def find_rust_files(self) -> List[Path]:
        """모든 Rust 소스 파일 찾기"""
        rust_files = []
        for root, dirs, files in os.walk(self.project_root):
            # target 디렉토리 제외
            if 'target' in dirs:
                dirs.remove('target')
            
            for file in files:
                if file.endswith('.rs'):
                    rust_files.append(Path(root) / file)
        
        return rust_files
    
    def analyze_unwrap(self, line: str) -> Tuple[bool, str]:
        """unwrap() 호출 분석 및 대체 코드 생성"""
        patterns = [
            # .unwrap() -> ?
            (r'\.unwrap\(\)', '?'),
            # .unwrap() with assignment -> .safe_unwrap()
            (r'let\s+(\w+)\s*=\s*(.*?)\.unwrap\(\)', 
             r'let \1 = \2.safe_unwrap("\1 initialization")?'),
            # .expect("msg") -> .safe_expect("msg")
            (r'\.expect\("([^"]+)"\)', r'.safe_expect("\1")?'),
        ]
        
        for pattern, replacement in patterns:
            if re.search(pattern, line):
                # 테스트 코드는 제외
                if '#[test]' in line or '#[cfg(test)]' in line:
                    return False, line
                
                new_line = re.sub(pattern, replacement, line)
                return True, new_line
        
        return False, line
    
    def process_file(self, filepath: Path) -> int:
        """파일 처리 및 unwrap 제거"""
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                lines = f.readlines()
            
            modified = False
            new_lines = []
            unwrap_count = 0
            in_test_block = False
            
            for line in lines:
                # 테스트 블록 감지
                if '#[test]' in line or '#[cfg(test)]' in line:
                    in_test_block = True
                elif in_test_block and line.strip() == '}':
                    in_test_block = False
                
                if not in_test_block and 'unwrap()' in line:
                    found, new_line = self.analyze_unwrap(line)
                    if found:
                        unwrap_count += 1
                        modified = True
                        new_lines.append(new_line)
                        self.stats['unwraps_fixed'] += 1
                    else:
                        new_lines.append(line)
                        if 'unwrap()' in line:
                            self.stats['unwraps_found'] += 1
                else:
                    new_lines.append(line)
            
            # 파일에 변경사항이 있으면 저장
            if modified:
                # use 문 추가 확인
                needs_import = False
                for line in new_lines:
                    if 'safe_unwrap' in line or 'safe_expect' in line:
                        needs_import = True
                        break
                
                if needs_import:
                    # error_handling 모듈 import 추가
                    insert_idx = 0
                    for i, line in enumerate(new_lines):
                        if line.startswith('use '):
                            insert_idx = i + 1
                        elif not line.startswith('//') and line.strip():
                            break
                    
                    import_line = 'use shared::error_handling::{SafeUnwrap, ProjectResult};\n'
                    if import_line not in new_lines:
                        new_lines.insert(insert_idx, import_line)
                
                with open(filepath, 'w', encoding='utf-8') as f:
                    f.writelines(new_lines)
                
                print(f"✅ {filepath.name}: {unwrap_count} unwrap() 수정됨")
            
            return unwrap_count
            
        except Exception as e:
            self.stats['errors'].append(f"{filepath}: {str(e)}")
            return 0
    
    def run(self):
        """전체 프로젝트 처리"""
        print("🔍 Rust 파일 검색 중...")
        rust_files = self.find_rust_files()
        print(f"📁 {len(rust_files)}개 파일 발견\n")
        
        for filepath in rust_files:
            self.stats['files_processed'] += 1
            self.process_file(filepath)
        
        # 결과 출력
        print("\n" + "="*50)
        print("📊 unwrap() 제거 결과")
        print("="*50)
        print(f"처리된 파일: {self.stats['files_processed']}")
        print(f"발견된 unwrap: {self.stats['unwraps_found']}")
        print(f"수정된 unwrap: {self.stats['unwraps_fixed']}")
        
        if self.stats['errors']:
            print(f"\n⚠️ 오류 발생: {len(self.stats['errors'])}개")
            for error in self.stats['errors'][:5]:
                print(f"  - {error}")

def main():
    if len(sys.argv) < 2:
        print("Usage: python remove_unwraps.py <project_root>")
        sys.exit(1)
    
    project_root = sys.argv[1]
    remover = UnwrapRemover(project_root)
    remover.run()

if __name__ == "__main__":
    main()