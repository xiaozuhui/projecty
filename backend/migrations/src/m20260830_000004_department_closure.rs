use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "WITH RECURSIVE department_tree AS (
                    SELECT id AS ancestor_id, id AS descendant_id, 0 AS depth
                    FROM departments
                    WHERE deleted_at IS NULL
                    UNION ALL
                    SELECT tree.ancestor_id, child.id AS descendant_id, tree.depth + 1
                    FROM department_tree tree
                    JOIN departments child ON child.parent_id = tree.descendant_id
                    WHERE child.deleted_at IS NULL
                )
                INSERT INTO department_closure (ancestor_id, descendant_id, depth)
                SELECT ancestor_id, descendant_id, depth
                FROM department_tree
                ON CONFLICT (ancestor_id, descendant_id) DO NOTHING",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_department_closure_descendant_ancestor")
                    .table(DepartmentClosure::Table)
                    .col(DepartmentClosure::DescendantId)
                    .col(DepartmentClosure::AncestorId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_user_departments_department_user")
                    .table(UserDepartments::Table)
                    .col(UserDepartments::DepartmentId)
                    .col(UserDepartments::UserId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_project_members_user_project")
                    .table(ProjectMembers::Table)
                    .col(ProjectMembers::UserId)
                    .col(ProjectMembers::ProjectId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_project_department_grants_department_project")
                    .table(ProjectDepartmentGrants::Table)
                    .col(ProjectDepartmentGrants::DepartmentId)
                    .col(ProjectDepartmentGrants::ProjectId)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for name in [
            "idx_project_department_grants_department_project",
            "idx_project_members_user_project",
            "idx_user_departments_department_user",
            "idx_department_closure_descendant_ancestor",
        ] {
            manager
                .drop_index(Index::drop().name(name).to_owned())
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum DepartmentClosure {
    Table,
    AncestorId,
    DescendantId,
}

#[derive(DeriveIden)]
enum UserDepartments {
    Table,
    DepartmentId,
    UserId,
}

#[derive(DeriveIden)]
enum ProjectMembers {
    Table,
    UserId,
    ProjectId,
}

#[derive(DeriveIden)]
enum ProjectDepartmentGrants {
    Table,
    DepartmentId,
    ProjectId,
}
