#!/usr/bin/env python3
"""
#[allow(...)] 경고 억제 제거 스크립트
"""

import re
import os
from pathlib import Path

def remove_allows(project_root: str):
    """프로젝트에서 모든 #[allow] 제거"""
    project_root = Path(project_root)
    count = 0
    
    # Rust 파일 찾기 (target 제외)
    for root, dirs, files in os.walk(project_root):
        if 'target' in dirs:
            dirs.remove('target')
        
        for file in files:
            if file.endswith('.rs'):
                filepath = Path(root) / file
                
                try:
                    with open(filepath, 'r', encoding='utf-8') as f:
                        content = f.read()
                    
                    original = content
                    
                    # #[allow(...)] 패턴 제거
                    patterns = [
                        r'^\s*#\[allow\([^)]*\)\]\s*\n',  # 전체 줄 제거
                        r'#\[allow\([^)]*\)\]\s*',  # 인라인 제거
                    ]
                    
                    for pattern in patterns:
                        content = re.sub(pattern, '', content, flags=re.MULTILINE)
                    
                    if content != original:
                        with open(filepath, 'w', encoding='utf-8') as f:
                            f.write(content)
                        
                        # 변경 횟수 계산
                        removed = len(re.findall(r'#\[allow\([^)]*\)\]', original))
                        count += removed
                        if removed > 0:
                            print(f"✅ {filepath.name}: {removed}개 #[allow] 제거")
                
                except Exception as e:
                    print(f"❌ {filepath}: {e}")
    
    print(f"\n총 {count}개 #[allow] 제거 완료")

if __name__ == "__main__":
    import sys
    if len(sys.argv) < 2:
        print("Usage: python remove_allows.py <project_root>")
        sys.exit(1)
    
    remove_allows(sys.argv[1])