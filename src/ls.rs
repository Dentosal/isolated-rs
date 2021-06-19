            let mut entries = std::fs::read_dir(".")
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            println!("{:?}", entries);
